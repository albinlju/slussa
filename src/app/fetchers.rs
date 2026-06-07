use tokio::task;

use crate::{
    app::{
        App,
        action::{Action, LoadedAction},
    },
    clients::github::{self, FetchError},
};

impl App {
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
        self.spawn_fetch(github::fetch_prs, |r| Action::Loaded(LoadedAction::Prs(r)));
    }

    pub(super) fn spawn_load_commits(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_commits(pr_id),
            move |r| Action::Loaded(LoadedAction::Commits(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_diff(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_diff(pr_id),
            move |r| Action::Loaded(LoadedAction::Diff(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_comments(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_comments(pr_id),
            move |r| Action::Loaded(LoadedAction::Comments(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_review_threads(&self, pr_id: u64) {
        self.spawn_fetch(
            move || github::fetch_review_threads(pr_id),
            move |r| Action::Loaded(LoadedAction::ReviewThreads(pr_id, r)),
        );
    }
}
