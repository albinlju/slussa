use crate::{
    app::{
        App,
        action::LoadedAction,
        store::{FetchKey, LoadState, Notice, Operation, PrData},
    },
    domain::pr::{PrBatch, PrStatus, PullRequest},
};

impl App {
    pub(super) fn loaded_actions(&mut self, action: LoadedAction) {
        let key = match &action {
            LoadedAction::Prs(_) => Some(FetchKey::Prs),
            LoadedAction::OlderPrs(_) => Some(FetchKey::OlderPrs),
            LoadedAction::Commits(id, _) => Some(FetchKey::Commits(*id)),
            LoadedAction::Diff(id, _) => Some(FetchKey::Diff(*id)),
            LoadedAction::Builds(id, _) => Some(FetchKey::Builds(*id)),
            LoadedAction::Activity(id, _) => Some(FetchKey::Activity(*id)),
            LoadedAction::Mergeability(id, _) => Some(FetchKey::Mergeability(*id)),
            LoadedAction::CommitDiff(id, oid, _) => Some(FetchKey::CommitDiff(*id, oid.clone())),
            _ => None,
        };
        if let Some(key) = &key {
            let failed = match &action {
                LoadedAction::Prs(r) | LoadedAction::OlderPrs(r) => r.is_err(),
                LoadedAction::Commits(_, r) => r.is_err(),
                LoadedAction::Diff(_, r) | LoadedAction::CommitDiff(_, _, r) => r.is_err(),
                LoadedAction::Builds(_, r) => r.is_err(),
                LoadedAction::Activity(_, r) => r.is_err(),
                LoadedAction::Mergeability(_, r) => r.is_err(),
                _ => false,
            };
            if failed && self.state.store.has_cached_data(key) {
                self.state.store.refresh_failures.insert(key.clone());
            } else if !failed {
                self.state.store.refresh_failures.remove(key);
            }
            self.state.store.fetches.remove(key);
        }
        match action {
            LoadedAction::ReviewFailed {
                pr_id,
                posted_comments,
                submitted_summary,
                message,
            } => {
                if let Some(review) = self.state.store.reviews.get_mut(&pr_id) {
                    let count = posted_comments.min(review.comments.len());
                    review.comments.drain(..count);
                    if submitted_summary.is_some() {
                        review.submitted_summary = submitted_summary;
                    }
                }
                self.pr_state_changed("review", pr_id, Err(message));
                self.reload_after_mutation(pr_id);
            }
            LoadedAction::OlderPrs(r) => self.older_prs_loaded(r),
            LoadedAction::Prs(r) => {
                log_outcome("prs", None, &r);
                let r = r.map(|batch| self.adopt_first_read(batch));
                let selected_id = self
                    .state
                    .ui
                    .list
                    .filtered_prs(&self.state.store.cache.prs, &self.state.store.current_user)
                    .get(self.state.ui.list.selected)
                    .map(|pr| pr.id);
                self.state.store.cache.prs.reload(r);
                let filtered = self
                    .state
                    .ui
                    .list
                    .filtered_prs(&self.state.store.cache.prs, &self.state.store.current_user);
                self.state.ui.list.selected = selected_id
                    .and_then(|id| filtered.iter().position(|pr| pr.id == id))
                    .unwrap_or_else(|| {
                        self.state
                            .ui
                            .list
                            .selected
                            .min(filtered.len().saturating_sub(1))
                    });
            }
            LoadedAction::Commits(pr_id, r) => {
                log_outcome("commits", Some(pr_id), &r);
                if let Ok(new) = &r {
                    let old = self
                        .state
                        .store
                        .cache
                        .details
                        .get(&pr_id)
                        .and_then(|data| match &data.commits {
                            LoadState::Loaded(commits) => Some(commits.as_slice()),
                            _ => None,
                        })
                        .unwrap_or(&[]);
                    self.state.ui.detail.reconcile_commits(pr_id, old, new);
                }
                self.pr_data_mut(pr_id).commits.reload(r);
            }
            LoadedAction::Diff(pr_id, r) => {
                log_outcome("diff", Some(pr_id), &r);
                self.pr_data_mut(pr_id).diff.reload(r);
            }
            LoadedAction::Builds(pr_id, r) => {
                log_outcome("builds", Some(pr_id), &r);
                self.pr_data_mut(pr_id).builds.reload(r);
            }
            LoadedAction::Activity(pr_id, r) => {
                log_outcome("activity", Some(pr_id), &r);
                self.pr_data_mut(pr_id).activity.reload(r);
            }
            LoadedAction::Mergeability(pr_id, r) => {
                log_outcome("mergeability", Some(pr_id), &r);
                self.pr_data_mut(pr_id).mergeability.reload(r);
            }
            LoadedAction::Merged(pr_id, r) => self.pr_state_changed("merge", pr_id, r),
            LoadedAction::Declined(pr_id, r) => self.pr_state_changed("decline", pr_id, r),
            LoadedAction::Reopened(pr_id, r) => self.pr_state_changed("reopen", pr_id, r),
            LoadedAction::CommitDiff(pr_id, oid, r) => {
                log_outcome("commit-diff", Some(pr_id), &r);
                self.pr_data_mut(pr_id)
                    .commit_diffs
                    .insert(oid, LoadState::from_result(r));
            }
            LoadedAction::Commented(pr_id, r) => self.pr_state_changed("comment", pr_id, r),
        }
        if let Some(key) = key
            && self.state.store.reload_after_fetch.remove(&key)
        {
            self.load_resource(key);
        }
    }

    fn pr_state_changed(&mut self, kind: &'static str, pr_id: u64, r: Result<(), String>) {
        log_outcome(kind, Some(pr_id), &r);
        let Some(operation) = self.state.store.operations.remove(&pr_id) else {
            return;
        };
        if matches!(operation, Operation::Comment | Operation::Review) {
            self.state.ui.detail.submission_finished(pr_id, r.is_ok());
        }
        match r {
            Ok(()) => {
                let label = match operation {
                    Operation::Merge => "merged",
                    Operation::Decline => "closed / declined",
                    Operation::Reopen => "reopened",
                    Operation::Review => "review submitted",
                    Operation::Comment => "comment saved",
                    Operation::Moderation => "comment / thread updated",
                };
                self.state.store.notice =
                    Some(Notice::new(format!("PR #{pr_id} · {label}"), false));
                self.state.store.uncertain_submissions.remove(&pr_id);
                self.state.store.errors.remove(&pr_id);
                if matches!(operation, Operation::Review) {
                    self.state.store.reviews.remove(&pr_id);
                }
                self.reload_after_mutation(pr_id);
            }
            Err(msg) => {
                self.state.store.uncertain_submissions.insert(pr_id);
                self.state.store.errors.insert(pr_id, msg);
            }
        }
    }

    /// A fresh first read becomes the list. Older closed PRs the user has
    /// already loaded stay, and so does the position to continue from; without
    /// that a refresh every minute would throw them away.
    fn adopt_first_read(&mut self, batch: PrBatch) -> Vec<PullRequest> {
        let mut prs = batch.prs;
        if !self.state.store.older_loaded {
            self.state.store.older_cursor = batch.more;
            return prs;
        }
        if let LoadState::Loaded(previous) = &self.state.store.cache.prs {
            let closed =
                |pr: &&PullRequest| matches!(pr.status, PrStatus::Merged | PrStatus::Declined);
            for old in previous.iter().filter(closed) {
                if !prs.iter().any(|pr| pr.id == old.id) {
                    prs.push(old.clone());
                }
            }
        }
        prs
    }

    fn older_prs_loaded(&mut self, result: Result<PrBatch, String>) {
        log_outcome("older-prs", None, &result);
        let notice = match result {
            Ok(batch) => {
                let mut added = 0;
                if let LoadState::Loaded(prs) = &mut self.state.store.cache.prs {
                    for pr in batch.prs {
                        if !prs.iter().any(|existing| existing.id == pr.id) {
                            prs.push(pr);
                            added += 1;
                        }
                    }
                }
                self.state.store.older_cursor = batch.more;
                self.state.store.older_loaded = true;
                let message = match (added, self.state.store.older_cursor.is_some()) {
                    (0, _) => "No older PRs".to_owned(),
                    (1, _) => "Loaded 1 older PR".to_owned(),
                    (n, _) => format!("Loaded {n} older PRs"),
                };
                Notice::new(message, false)
            }
            Err(message) => Notice::new(format!("Couldn't load older PRs: {message}"), true),
        };
        self.state.store.notice = Some(notice);
    }

    fn pr_data_mut(&mut self, pr_id: u64) -> &mut PrData {
        self.state.store.cache.details.entry(pr_id).or_default()
    }
}

fn log_outcome<T>(kind: &'static str, pr_id: Option<u64>, result: &Result<T, String>) {
    match result {
        Ok(_) => tracing::debug!("fetch loaded: {kind} pr={pr_id:?}"),
        Err(e) => tracing::warn!("fetch failed: {kind} pr={pr_id:?}: {e}"),
    }
}

impl App {
    pub(super) fn ensure_commit_diff(&mut self, pr_id: u64, oid: String) {
        let data = self.state.store.cache.details.entry(pr_id).or_default();
        if data
            .commit_diffs
            .entry(oid.clone())
            .or_insert(LoadState::NotRequested)
            .start_loading()
        {
            self.spawn_load_commit_diff(pr_id, oid);
        }
    }
}
