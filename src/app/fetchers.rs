use tokio::task;

use crate::{
    app::{
        App,
        action::{Action, LoadedAction},
        state::{CommentTarget, PendingComment},
    },
    domain::review::{ReviewComment, ReviewVerdict},
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
                Ok(r) => r.map_err(|e| e.user_message()),
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

    pub(super) fn spawn_comment(&self, pr_id: u64, target: CommentTarget, text: String) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || match target {
                CommentTarget::Line(a) => {
                    provider.post_comment(pr_id, &a.path, a.line, a.removed, &text)
                }
                CommentTarget::Pr => provider.post_pr_comment(pr_id, &text),
                CommentTarget::Reply(parent) => provider.reply_comment(pr_id, parent, &text),
                CommentTarget::Edit { id, review } => {
                    provider.edit_comment(pr_id, id, review, &text)
                }
                // Review verdicts are routed to `spawn_submit_review` upstream.
                CommentTarget::Review { .. } => unreachable!("review target handled separately"),
            },
            move |r| Action::Loaded(LoadedAction::Commented(pr_id, r)),
        );
    }

    pub(super) fn spawn_delete_comment(&self, pr_id: u64, comment_id: u64, review: bool) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.delete_comment(pr_id, comment_id, review),
            move |r| Action::Loaded(LoadedAction::Commented(pr_id, r)),
        );
    }

    pub(super) fn spawn_submit_review(
        &self,
        pr_id: u64,
        verdict: ReviewVerdict,
        body: String,
        user: String,
    ) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.submit_review(pr_id, verdict, &body, &user),
            move |r| Action::Loaded(LoadedAction::Commented(pr_id, r)),
        );
    }

    /// Flush a whole review at once: the queued line `comments` plus the
    /// `verdict` and its summary `body`. GitHub sends one atomic call; Bitbucket
    /// posts the comments then flips status (see the provider impls).
    pub(super) fn spawn_submit_full_review(
        &self,
        pr_id: u64,
        verdict: ReviewVerdict,
        body: String,
        user: String,
        comments: Vec<PendingComment>,
    ) {
        let provider = self.provider.clone();
        let review_comments: Vec<ReviewComment> = comments
            .into_iter()
            .map(|c| ReviewComment {
                path: c.anchor.path,
                line: c.anchor.line,
                removed: c.anchor.removed,
                body: c.text,
            })
            .collect();
        self.spawn_fetch(
            move || provider.submit_full_review(pr_id, verdict, &body, &user, &review_comments),
            move |r| Action::Loaded(LoadedAction::Commented(pr_id, r)),
        );
    }

    pub(super) fn spawn_resolve_thread(
        &self,
        pr_id: u64,
        node_id: Option<String>,
        comment_id: Option<u64>,
        resolved: bool,
    ) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.set_thread_resolved(pr_id, node_id.as_deref(), comment_id, resolved),
            move |r| Action::Loaded(LoadedAction::Commented(pr_id, r)),
        );
    }
}
