use crate::{
    app::{
        App,
        action::LoadedAction,
        navigation::Screen,
        store::{FetchKey, LoadState, Notice, OpenChain, Operation, PrData},
    },
    domain::pr::{PrBatch, PrGroup, PrStatus},
};

impl App {
    pub(super) fn loaded_actions(&mut self, action: LoadedAction) {
        let key = match &action {
            LoadedAction::Prs { group, .. } => Some(FetchKey::Prs(*group)),
            LoadedAction::Commits(id, _) => Some(FetchKey::Commits(*id)),
            LoadedAction::Diff(id, _) => Some(FetchKey::Diff(*id)),
            LoadedAction::Builds(id, _) => Some(FetchKey::Builds(*id)),
            LoadedAction::Activity(id, _) => Some(FetchKey::Activity(*id)),
            LoadedAction::Mergeability(id, _) => Some(FetchKey::Mergeability(*id)),
            LoadedAction::Info(id, _) => Some(FetchKey::Info(*id)),
            LoadedAction::CommitDiff(id, oid, _) => Some(FetchKey::CommitDiff(*id, oid.clone())),
            _ => None,
        };
        if let Some(key) = &key {
            let failed = match &action {
                LoadedAction::Prs { result, .. } => result.is_err(),
                LoadedAction::Commits(_, r) => r.is_err(),
                LoadedAction::Diff(_, r) | LoadedAction::CommitDiff(_, _, r) => r.is_err(),
                LoadedAction::Builds(_, r) => r.is_err(),
                LoadedAction::Activity(_, r) => r.is_err(),
                LoadedAction::Mergeability(_, r) => r.is_err(),
                LoadedAction::Info(_, r) => r.is_err(),
                _ => false,
            };
            // A failed read of an older page says so in its own notice and
            // is not a stale list.
            let older_page = matches!(
                &action,
                LoadedAction::Prs { group, after: Some(_), .. } if group.is_paged()
            );
            if failed && !older_page && self.state.store.has_cached_data(key) {
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
            LoadedAction::Prs {
                group,
                after,
                result,
            } => self.prs_loaded(group, after.is_some(), result),
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
            LoadedAction::Info(pr_id, r) => {
                log_outcome("info", Some(pr_id), &r);
                self.pr_data_mut(pr_id).info.reload(r);
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
                // Show the new status at once; the refetch below confirms it.
                // Without this the PR would vanish from the list until the
                // group it moved to is read.
                let moved_to = match operation {
                    Operation::Merge => Some(PrStatus::Merged),
                    Operation::Decline => Some(PrStatus::Declined),
                    Operation::Reopen => Some(PrStatus::Open),
                    _ => None,
                };
                if let Some(status) = moved_to
                    && let LoadState::Loaded(prs) = &mut self.state.store.cache.prs
                    && let Some(pr) = prs.iter_mut().find(|pr| pr.id == pr_id)
                {
                    pr.status = status;
                }
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

    fn prs_loaded(&mut self, group: PrGroup, continuation: bool, result: Result<PrBatch, String>) {
        let older = continuation && group.is_paged();
        log_outcome("prs", None, &result);
        let rows = |app: &Self| {
            app.state
                .ui
                .list
                .filtered_prs(&app.state.store.cache.prs, &app.state.store.current_user)
                .get(app.state.ui.list.selected)
                .map(|pr| pr.id)
        };
        let selected_id = rows(self);
        if group == PrGroup::Open && result.is_err() {
            self.state.store.open_chain = OpenChain::Idle;
            self.state.ui.list.hold_order = false;
        }
        match result {
            Ok(batch) if group == PrGroup::Open => self.adopt_open_page(continuation, batch),
            Ok(batch) => self.adopt_group(group, older, batch),
            Err(message) if older => {
                self.state.store.notice = Some(Notice::new(
                    format!("Couldn't load older PRs: {message}"),
                    true,
                ));
            }
            // The open group is the list itself: a first failure shows as a
            // failed list, a failed refresh keeps what is there.
            Err(message) if group == PrGroup::Open => {
                self.state.store.cache.prs.reload(Err(message));
            }
            // A closed group never read before has no data to keep, so say so.
            Err(message) if !self.state.store.group_loaded(group) => {
                self.state.store.notice = Some(Notice::new(
                    format!("Couldn't load {} PRs: {message}", group.label()),
                    true,
                ));
            }
            // A failed refresh of a group already read is recorded in
            // `refresh_failures` above.
            Err(_) => {}
        }
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

    /// Take a group's read into the list. A first read replaces the group's
    /// PRs; older closed PRs already loaded stay, and so does the position
    /// reached, or a refresh every minute would throw them away. An older page
    /// goes after what is there. The PR open in the detail screen is never
    /// dropped, so a merge by someone else does not blank the screen.
    /// One page of the open group. A first reading shows each page as it
    /// arrives, in arrival order. A refresh holds the pages and swaps them in
    /// after the last one. Either way the next page is asked for until there is
    /// none or the limit is reached, and then the list takes its order.
    fn adopt_open_page(&mut self, continuation: bool, batch: PrBatch) {
        let more = batch.more.clone();
        let store = &mut self.state.store;
        let chain = match (continuation, std::mem::take(&mut store.open_chain)) {
            (false, _) if matches!(store.cache.prs, LoadState::Loaded(_)) => {
                OpenChain::Collecting(batch.prs)
            }
            (false, _) => {
                self.adopt_group(PrGroup::Open, false, batch);
                OpenChain::Appending
            }
            (true, OpenChain::Appending) => {
                if let LoadState::Loaded(prs) = &mut store.cache.prs {
                    for pr in batch.prs {
                        if !prs.iter().any(|known| known.id == pr.id) {
                            prs.push(pr);
                        }
                    }
                    prs.sort_by_key(|pr| PrGroup::of(&pr.status));
                }
                OpenChain::Appending
            }
            (true, OpenChain::Collecting(mut held)) => {
                held.extend(batch.prs);
                OpenChain::Collecting(held)
            }
            // A page that belongs to a reading that has already ended.
            (true, OpenChain::Idle) => return,
        };
        let store = &mut self.state.store;
        let read = match &chain {
            OpenChain::Collecting(held) => held.len(),
            _ => match &store.cache.prs {
                LoadState::Loaded(prs) => prs
                    .iter()
                    .filter(|pr| PrGroup::of(&pr.status) == PrGroup::Open)
                    .count(),
                _ => 0,
            },
        };
        let next = more.clone().filter(|_| read < store.open_limit());
        // Where `L` continues from, if the reading stops with more left.
        store
            .groups
            .entry(PrGroup::Open)
            .or_default()
            .more
            .clone_from(&more);
        if let Some(after) = next {
            self.state.ui.list.hold_order = matches!(chain, OpenChain::Appending);
            self.state.store.open_chain = chain;
            self.spawn_load_prs(PrGroup::Open, Some(after));
            return;
        }
        if let OpenChain::Collecting(held) = chain {
            self.adopt_group(PrGroup::Open, false, PrBatch { prs: held, more });
        }
        self.state.ui.list.hold_order = false;
    }

    fn adopt_group(&mut self, group: PrGroup, older: bool, batch: PrBatch) {
        let viewing = match self.state.screen {
            Screen::Detail { pr_id, .. } => Some(pr_id),
            Screen::List => None,
        };
        let store = &mut self.state.store;
        let existing = match &mut store.cache.prs {
            LoadState::Loaded(prs) => std::mem::take(prs),
            _ => Vec::new(),
        };
        let state = store.groups.entry(group).or_default();
        let mut notice = None;
        let mut prs = if older {
            let mut prs = existing;
            let mut added = 0;
            for pr in batch.prs {
                if !prs.iter().any(|known| known.id == pr.id) {
                    prs.push(pr);
                    added += 1;
                }
            }
            state.more = batch.more;
            state.older_loaded = true;
            notice = Some(match added {
                0 => "No older PRs".to_owned(),
                1 => "Loaded 1 older PR".to_owned(),
                n => format!("Loaded {n} older PRs"),
            });
            prs
        } else {
            let keep_older = state.older_loaded && group.is_paged();
            let mut prs = batch.prs;
            for old in existing {
                if prs.iter().any(|fresh| fresh.id == old.id) {
                    continue;
                }
                let in_group = PrGroup::of(&old.status) == group;
                if !in_group || keep_older || Some(old.id) == viewing {
                    prs.push(old);
                }
            }
            if !state.older_loaded {
                state.more = batch.more;
            }
            prs
        };
        state.loaded = true;
        // Providers give each group newest first; keep the groups in a fixed order.
        prs.sort_by_key(|pr| PrGroup::of(&pr.status));
        store.cache.prs = LoadState::Loaded(prs);
        if let Some(message) = notice {
            store.notice = Some(Notice::new(message, false));
        }
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
