use tokio::task;

use crate::{
    app::{
        App,
        action::{Action, LoadedAction},
        state::CommentAnchor,
    },
    providers::FetchError,
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
            let result = match task::spawn_blocking(fetch).await {
                Ok(r) => r.map_err(|e| e.to_string()),
                Err(join_err) => Err(format!("worker thread panicked: {join_err}")),
            };
            tx.send(make_action(result)).ok();
        });
    }

    pub(super) fn spawn_load_prs(&self) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_prs(),
            |r| Action::Loaded(LoadedAction::Prs(r)),
        );
    }

    pub(super) fn spawn_load_commits(&self, pr_id: u64) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_commits(pr_id),
            move |r| Action::Loaded(LoadedAction::Commits(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_diff(&self, pr_id: u64) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_diff(pr_id),
            move |r| Action::Loaded(LoadedAction::Diff(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_builds(&self, pr_id: u64) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_builds(pr_id),
            move |r| Action::Loaded(LoadedAction::Builds(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_commit_diff(&self, pr_id: u64, oid: String) {
        let provider = self.provider.clone();
        let oid_fetch = oid.clone();
        self.spawn_fetch(
            move || provider.fetch_commit_diff(&oid_fetch),
            move |r| Action::Loaded(LoadedAction::CommitDiff(pr_id, oid, r)),
        );
    }

    pub(super) fn spawn_load_activity(&self, pr_id: u64) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_activity(pr_id),
            move |r| Action::Loaded(LoadedAction::Activity(pr_id, r)),
        );
    }

    pub(super) fn spawn_comment(&self, pr_id: u64, anchor: CommentAnchor, text: String) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.post_comment(pr_id, &anchor.path, anchor.line, anchor.removed, &text),
            move |r| Action::Loaded(LoadedAction::Commented(pr_id, r)),
        );
    }
}
