use tokio::task;

use crate::{
    app::{
        App,
        action::{Action, LoadedAction},
    },
    clients::FetchError,
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
        let backend = self.backend.clone();
        self.spawn_fetch(
            move || backend.fetch_prs(),
            |r| Action::Loaded(LoadedAction::Prs(r)),
        );
    }

    pub(super) fn spawn_load_commits(&self, pr_id: u64) {
        let backend = self.backend.clone();
        self.spawn_fetch(
            move || backend.fetch_commits(pr_id),
            move |r| Action::Loaded(LoadedAction::Commits(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_diff(&self, pr_id: u64) {
        let backend = self.backend.clone();
        self.spawn_fetch(
            move || backend.fetch_diff(pr_id),
            move |r| Action::Loaded(LoadedAction::Diff(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_builds(&self, pr_id: u64) {
        let backend = self.backend.clone();
        self.spawn_fetch(
            move || backend.fetch_builds(pr_id),
            move |r| Action::Loaded(LoadedAction::Builds(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_activity(&self, pr_id: u64) {
        let backend = self.backend.clone();
        self.spawn_fetch(
            move || backend.fetch_activity(pr_id),
            move |r| Action::Loaded(LoadedAction::Activity(pr_id, r)),
        );
    }
}
