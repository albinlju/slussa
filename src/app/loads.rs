use crate::app::{
    App,
    action::LoadedAction,
    store::{FetchKey, LoadState, Operation, PrData},
};

impl App {
    pub(super) fn loaded_actions(&mut self, action: LoadedAction) {
        let key = match &action {
            LoadedAction::Prs(_) => Some(FetchKey::Prs),
            LoadedAction::Commits(id, _) => Some(FetchKey::Commits(*id)),
            LoadedAction::Diff(id, _) => Some(FetchKey::Diff(*id)),
            LoadedAction::Builds(id, _) => Some(FetchKey::Builds(*id)),
            LoadedAction::Activity(id, _) => Some(FetchKey::Activity(*id)),
            LoadedAction::Mergeability(id, _) => Some(FetchKey::Mergeability(*id)),
            LoadedAction::CommitDiff(id, oid, _) => Some(FetchKey::CommitDiff(*id, oid.clone())),
            _ => None,
        };
        if let Some(key) = &key {
            self.state.store.fetches.remove(key);
        }
        match action {
            LoadedAction::Prs(r) => {
                log_outcome("prs", None, &r);
                self.state.store.cache.prs.reload(r);
            }
            LoadedAction::Commits(pr_id, r) => {
                log_outcome("commits", Some(pr_id), &r);
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
                self.state.store.errors.remove(&pr_id);
                if matches!(operation, Operation::Review) {
                    self.state.store.reviews.remove(&pr_id);
                }
                self.reload_after_mutation(pr_id);
            }
            Err(msg) => {
                self.state.store.errors.insert(pr_id, msg);
            }
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
