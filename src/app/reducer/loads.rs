//! Handlers for `LoadedAction` — the messages background fetchers send back
//! with the fetch result. Each variant threads `Result<T, String>` through
//! `LoadState`'s `From` impl so the cache flips into `Loaded` or `Failed`.

use crate::{app::App, tui::LoadedAction};

impl App {
    pub(super) fn apply_loaded(&mut self, action: LoadedAction) {
        match action {
            LoadedAction::Prs(r) => {
                self.state.cache.prs = r.into();
            }
            LoadedAction::Commits(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.commits = r.into();
            }
            LoadedAction::Diff(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.diff = r.into();
            }
            LoadedAction::Comments(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.comments = r.into();
            }
            LoadedAction::ReviewThreads(pr_id, r) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.review_threads = r.into();
            }
        }
    }
}
