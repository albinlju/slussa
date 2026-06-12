use crate::app::{
    App,
    action::LoadedAction,
    state::{LoadState, PrData},
};

impl App {
    pub(super) fn apply_loaded(&mut self, action: LoadedAction) {
        match action {
            LoadedAction::Prs(r) => {
                log_outcome("prs", None, &r);
                self.state.cache.prs = LoadState::from_result(r);
            }
            LoadedAction::Commits(pr_id, r) => {
                log_outcome("commits", Some(pr_id), &r);
                self.pr_data_mut(pr_id).commits = LoadState::from_result(r);
            }
            LoadedAction::Diff(pr_id, r) => {
                log_outcome("diff", Some(pr_id), &r);
                self.pr_data_mut(pr_id).diff = LoadState::from_result(r);
            }
            LoadedAction::Builds(pr_id, r) => {
                log_outcome("builds", Some(pr_id), &r);
                self.pr_data_mut(pr_id).builds = LoadState::from_result(r);
            }
            LoadedAction::Activity(pr_id, r) => {
                log_outcome("activity", Some(pr_id), &r);
                self.pr_data_mut(pr_id).activity = LoadState::from_result(r);
            }
            LoadedAction::CommitDiff(pr_id, oid, r) => {
                log_outcome("commit-diff", Some(pr_id), &r);
                self.pr_data_mut(pr_id)
                    .commit_diffs
                    .insert(oid, LoadState::from_result(r));
            }
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
