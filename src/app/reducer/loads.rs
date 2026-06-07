//! Handlers for `LoadedAction` — the messages background fetchers send back
//! with the fetch result. Each variant lands its `Result<T, String>` in the
//! cache via `LoadState::from_result`.

use crate::app::{App, action::LoadedAction, state::LoadState};

impl App {
    pub(super) fn apply_loaded(&mut self, action: LoadedAction) {
        match action {
            LoadedAction::Prs(r) => {
                self.state.cache.prs = LoadState::from_result(r);
            }
            LoadedAction::Commits(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.commits = LoadState::from_result(r);
            }
            LoadedAction::Diff(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.diff = LoadState::from_result(r);
            }
            LoadedAction::Comments(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.comments = LoadState::from_result(r);
            }
            LoadedAction::ReviewThreads(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.review_threads = LoadState::from_result(r);
            }
        }
    }
}
