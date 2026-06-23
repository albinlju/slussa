use crate::app::{
    App,
    action::LoadedAction,
    state::{LoadState, PrData},
};

impl App {
    pub(super) fn loaded_actions(&mut self, action: LoadedAction) {
        // Any settled fetch clears the footer's refresh indicator.
        self.state.ui.refreshing = false;
        match action {
            LoadedAction::Prs(r) => {
                log_outcome("prs", None, &r);
                self.state.cache.prs.reload(r);
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
                self.state.ui.comment_pending = false;
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
            LoadedAction::Commented(pr_id, r) => {
                log_outcome("comment", Some(pr_id), &r);
                match r {
                    Ok(()) => self.spawn_load_activity(pr_id),
                    Err(msg) => {
                        self.state.ui.comment_pending = false;
                        self.state.ui.error = Some(msg);
                    }
                }
            }
        }
    }

    /// A merge or decline changed the PR's lifecycle: refetch the list (status),
    /// activity, and mergeability so the header reflects it. Shared by both.
    fn pr_state_changed(&mut self, kind: &'static str, pr_id: u64, r: Result<(), String>) {
        log_outcome(kind, Some(pr_id), &r);
        self.state.ui.comment_pending = false;
        match r {
            Ok(()) => {
                self.spawn_load_prs();
                self.spawn_load_activity(pr_id);
                self.spawn_load_mergeability(pr_id);
            }
            Err(msg) => self.state.ui.error = Some(msg),
        }
    }

    fn pr_data_mut(&mut self, pr_id: u64) -> &mut PrData {
        self.state.cache.details.entry(pr_id).or_default()
    }
}

fn log_outcome<T>(kind: &'static str, pr_id: Option<u64>, result: &Result<T, String>) {
    match result {
        Ok(_) => tracing::debug!("fetch loaded: {kind} pr={pr_id:?}"),
        Err(e) => tracing::warn!("fetch failed: {kind} pr={pr_id:?}: {e}"),
    }
}
