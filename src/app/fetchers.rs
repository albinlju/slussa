use tokio::task;

use crate::{
    app::App,
    providers::github::{self, FetchError},
    tui::Action,
};

impl App {
    /// Run a `gh` fetch on a blocking worker, then route its `Result<T, _>`
    /// through `make_action` onto the UI thread. Worker panics surface as
    /// `ParseFailed("worker thread panicked: …")` so a Failed `LoadState`
    /// shows the user something instead of an empty result.
    fn spawn_fetch<T, F, A>(&self, fetch: F, make_action: A)
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, FetchError> + Send + 'static,
        A: FnOnce(Result<T, String>) -> Action + Send + 'static,
    {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let result = task::spawn_blocking(fetch)
                .await
                .unwrap_or_else(|join_err| {
                    Err(FetchError::ParseFailed(format!(
                        "worker thread panicked: {join_err}"
                    )))
                })
                .map_err(|e| e.to_string());
            tx.send(make_action(result)).ok();
        });
    }

    pub(super) fn spawn_load_prs(&self) {
        self.spawn_fetch(github::fetch_prs, Action::PrsLoaded);
    }

    pub(super) fn spawn_load_commits(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_commits(pr_id),
            move |r| Action::CommitsLoaded(pr_id, r),
        );
    }

    pub(super) fn spawn_load_diff(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_diff(pr_id),
            move |r| Action::DiffLoaded(pr_id, r),
        );
    }

    pub(super) fn spawn_load_comments(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_comments(pr_id),
            move |r| Action::CommentsLoaded(pr_id, r),
        );
    }

    pub(super) fn spawn_load_review_threads(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_review_threads(pr_id),
            move |r| Action::ReviewThreadsLoaded(pr_id, r),
        );
    }
}
