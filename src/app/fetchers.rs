use crate::{
    app::{
        App,
        action::{Action, LoadedAction},
        reviews::{CommentTarget, PendingComment},
        store::FetchKey,
    },
    domain::{
        pr::MergeStrategy,
        review::{ReviewComment, ReviewVerdict},
    },
    providers::FetchError,
};
use tokio::task;

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

    pub(super) fn spawn_load_prs(&mut self) {
        if !self.state.store.fetches.insert(FetchKey::Prs) {
            return;
        }
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_prs(),
            |r| Action::Loaded(LoadedAction::Prs(r)),
        );
    }

    pub(super) fn spawn_load_commits(&mut self, pr_id: u64) {
        if !self.state.store.fetches.insert(FetchKey::Commits(pr_id)) {
            return;
        }
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_commits(pr_id),
            move |r| Action::Loaded(LoadedAction::Commits(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_diff(&mut self, pr_id: u64) {
        if !self.state.store.fetches.insert(FetchKey::Diff(pr_id)) {
            return;
        }
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_diff(pr_id),
            move |r| Action::Loaded(LoadedAction::Diff(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_builds(&mut self, pr_id: u64) {
        if !self
            .state
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Builds)
        {
            return;
        }
        if !self.state.store.fetches.insert(FetchKey::Builds(pr_id)) {
            return;
        }
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_builds(pr_id),
            move |r| Action::Loaded(LoadedAction::Builds(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_commit_diff(&mut self, pr_id: u64, oid: String) {
        if !self
            .state
            .store
            .fetches
            .insert(FetchKey::CommitDiff(pr_id, oid.clone()))
        {
            return;
        }
        let provider = self.provider.clone();
        let oid_fetch = oid.clone();
        self.spawn_fetch(
            move || provider.fetch_commit_diff(&oid_fetch),
            move |r| Action::Loaded(LoadedAction::CommitDiff(pr_id, oid, r)),
        );
    }

    pub(super) fn spawn_load_activity(&mut self, pr_id: u64) {
        if !self.state.store.fetches.insert(FetchKey::Activity(pr_id)) {
            return;
        }
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_activity(pr_id),
            move |r| Action::Loaded(LoadedAction::Activity(pr_id, r)),
        );
    }

    pub(super) fn spawn_merge(&self, pr_id: u64, strategy: MergeStrategy) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.merge(pr_id, strategy),
            move |r| Action::Loaded(LoadedAction::Merged(pr_id, r)),
        );
    }

    pub(super) fn spawn_decline(&self, pr_id: u64) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.decline(pr_id),
            move |r| Action::Loaded(LoadedAction::Declined(pr_id, r)),
        );
    }

    pub(super) fn spawn_load_mergeability(&mut self, pr_id: u64) {
        if !self
            .state
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Mergeability)
        {
            return;
        }
        if !self
            .state
            .store
            .fetches
            .insert(FetchKey::Mergeability(pr_id))
        {
            return;
        }
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || provider.fetch_mergeability(pr_id),
            move |r| Action::Loaded(LoadedAction::Mergeability(pr_id, r)),
        );
    }

    pub(super) fn spawn_comment(&self, pr_id: u64, target: CommentTarget, text: String) {
        let provider = self.provider.clone();
        self.spawn_fetch(
            move || match target {
                CommentTarget::Line(a) => provider.post_comment(pr_id, &a, &text),
                CommentTarget::Pr => provider.post_pr_comment(pr_id, &text),
                CommentTarget::Reply(parent) => provider.reply_comment(pr_id, parent, &text),
                CommentTarget::Edit { id, review } => {
                    provider.edit_comment(pr_id, id, review, &text)
                }
                // Review verdicts are routed to `spawn_submit_review` upstream.
                CommentTarget::Review { .. } => Err(FetchError::InvalidInput(
                    "A review verdict cannot be posted as a plain comment.".into(),
                )),
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
                revision: c.anchor.revision,
                path: c.anchor.path,
                line: c.anchor.line,
                removed: c.anchor.removed,
                body: c.text,
            })
            .collect();
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let submitted_body = body.clone();
            let result = task::spawn_blocking(move || {
                provider.submit_full_review(pr_id, verdict, &body, &user, &review_comments)
            })
            .await;
            let action = match result {
                Ok(Ok(())) => LoadedAction::Commented(pr_id, Ok(())),
                Ok(Err(FetchError::PartialReview {
                    posted_comments,
                    summary_posted,
                    source,
                })) => LoadedAction::ReviewFailed {
                    pr_id,
                    posted_comments,
                    submitted_summary: summary_posted.then_some(submitted_body),
                    message: format!(
                        "{}\n{posted_comments} line comments were sent; confirmed posts will be skipped on retry. Check the last attempted post before retrying.",
                        source.user_message()
                    ),
                },
                Ok(Err(error)) => LoadedAction::Commented(pr_id, Err(error.user_message())),
                Err(error) => {
                    LoadedAction::Commented(pr_id, Err(format!("worker thread panicked: {error}")))
                }
            };
            tx.send(Action::Loaded(action)).ok();
        });
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

impl App {
    /// A mutation must be followed by a fetch started after its acknowledgement.
    fn reload_resource(&mut self, key: FetchKey) {
        if self.state.store.fetches.contains(&key) {
            self.state.store.reload_after_fetch.insert(key);
        } else {
            self.load_resource(key);
        }
    }

    pub(super) fn reload_after_mutation(&mut self, pr_id: u64) {
        self.reload_resource(FetchKey::Activity(pr_id));
        self.reload_resource(FetchKey::Prs);
        self.reload_resource(FetchKey::Mergeability(pr_id));
    }

    pub(super) fn load_resource(&mut self, key: FetchKey) {
        match key {
            FetchKey::Prs => self.spawn_load_prs(),
            FetchKey::Commits(id) => self.spawn_load_commits(id),
            FetchKey::Diff(id) => self.spawn_load_diff(id),
            FetchKey::Builds(id) => self.spawn_load_builds(id),
            FetchKey::Activity(id) => self.spawn_load_activity(id),
            FetchKey::Mergeability(id) => self.spawn_load_mergeability(id),
            FetchKey::CommitDiff(id, oid) => self.spawn_load_commit_diff(id, oid),
        }
    }
}
