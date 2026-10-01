//! Work that leaves the UI thread. `spawn_fetch` is the one way out; a read
//! goes through `spawn_read` with the `FetchTicket` that registered it, a
//! write through `spawn_write` with its `WriteTicket`.
use crate::{
    app::{
        App,
        action::{Read, TaskResult, WriteError},
        reviews::{CommentTarget, PendingComment},
        store::{FetchKey, FetchTicket, OpenChain, WriteTicket},
    },
    domain::{
        pr::{MergeStrategy, PrGroup},
        review::{ReviewComment, ReviewVerdict},
    },
    providers::FetchError,
};
use tokio::task::{self, JoinError};

fn worker_panicked(panic: &JoinError) -> FetchError {
    FetchError::WorkerPanicked(panic.to_string())
}

impl App {
    /// Run `work`, which blocks, on a worker thread and send back what `done`
    /// makes of it. A worker that panicked gives `done` that instead.
    pub(super) fn spawn_fetch<R, W, D>(&self, work: W, done: D)
    where
        R: Send + 'static,
        W: FnOnce() -> R + Send + 'static,
        D: FnOnce(Result<R, JoinError>) -> TaskResult + Send + 'static,
    {
        let tx = self.results_tx.clone();
        tokio::spawn(async move {
            let returned = task::spawn_blocking(work).await;
            tx.send(done(returned)).ok();
        });
    }

    fn spawn_read<T, F, R>(&self, ticket: FetchTicket, fetch: F, read: R)
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, FetchError> + Send + 'static,
        R: FnOnce(Result<T, FetchError>) -> Read + Send + 'static,
    {
        self.spawn_fetch(fetch, move |returned| {
            let read = read(returned.unwrap_or_else(|panic| Err(worker_panicked(&panic))));
            if read.key() != *ticket.key() {
                tracing::error!("read of {:?} answered {:?}", ticket.key(), read.key());
            }
            TaskResult::Read(read)
        });
    }

    fn spawn_write<E, F>(&self, ticket: WriteTicket, write: F)
    where
        E: Into<WriteError> + Send + 'static,
        F: FnOnce() -> Result<(), E> + Send + 'static,
    {
        self.spawn_fetch(write, move |returned| TaskResult::Written {
            ticket,
            result: match returned {
                Ok(result) => result.map_err(Into::into),
                Err(panic) => Err(worker_panicked(&panic).into()),
            },
        });
    }

    /// Read one group of PRs, or with `after` the next page of a closed group,
    /// unless that group is already being read.
    pub(super) fn spawn_load_prs(&mut self, group: PrGroup, after: Option<String>) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Prs(group)) else {
            return;
        };
        let provider = self.provider.clone();
        let position = after.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_prs(group, position.as_deref()),
            move |result| Read::Prs {
                group,
                after,
                result,
            },
        );
    }

    /// Read whatever the current view shows that has not been read yet.
    pub(super) fn ensure_view_loaded(&mut self) {
        for &group in self.state.ui.list.filter.groups() {
            if !self.state.store.group_loaded(group) {
                self.spawn_load_prs(group, None);
            }
        }
    }

    /// `L`: the next batch of each group in the view that has more.
    pub(super) fn load_older_prs(&mut self) {
        for &group in self.state.ui.list.filter.groups() {
            if group == PrGroup::Open {
                self.load_more_open();
                continue;
            }
            let more = self
                .state
                .store
                .groups
                .get(&group)
                .and_then(|state| state.more.clone());
            if let Some(after) = more {
                self.spawn_load_prs(group, Some(after));
            }
        }
    }

    /// Read on in the open group, the next batch past what is shown.
    fn load_more_open(&mut self) {
        let store = &mut self.state.store;
        let Some(after) = store
            .groups
            .get(&PrGroup::Open)
            .and_then(|state| state.more.clone())
        else {
            return;
        };
        if store.group_loading(PrGroup::Open) || !matches!(store.open_chain, OpenChain::Idle) {
            return;
        }
        store.open_extra += 1;
        store.open_chain = OpenChain::Appending;
        self.state.ui.list.hold_order = true;
        self.spawn_load_prs(PrGroup::Open, Some(after));
    }

    /// Read the open group again, and every other group already read.
    pub(super) fn refresh_list(&mut self) {
        for group in PrGroup::ALL {
            if group == PrGroup::Open || self.state.store.group_loaded(group) {
                self.spawn_load_prs(group, None);
            }
        }
    }

    pub(super) fn spawn_load_commits(&mut self, pr_id: u64) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Commits(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_commits(pr_id),
            move |r| Read::Commits(pr_id, r),
        );
    }

    pub(super) fn spawn_load_diff(&mut self, pr_id: u64) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Diff(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_diff(pr_id),
            move |r| Read::Diff(pr_id, r),
        );
    }

    pub(super) fn spawn_load_builds(&mut self, pr_id: u64) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Builds(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_builds(pr_id),
            move |r| Read::Builds(pr_id, r),
        );
    }

    pub(super) fn spawn_load_commit_diff(&mut self, pr_id: u64, oid: String) {
        let key = FetchKey::CommitDiff(pr_id, oid.clone());
        let Some(ticket) = self.state.store.begin_fetch(key) else {
            return;
        };
        let provider = self.provider.clone();
        let oid_fetch = oid.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_commit_diff(&oid_fetch),
            move |r| Read::CommitDiff(pr_id, oid, r),
        );
    }

    pub(super) fn spawn_load_activity(&mut self, pr_id: u64) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Activity(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_activity(pr_id),
            move |r| Read::Activity(pr_id, r),
        );
    }

    pub(super) fn spawn_load_mergeability(&mut self, pr_id: u64) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Mergeability(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_mergeability(pr_id),
            move |r| Read::Mergeability(pr_id, r),
        );
    }

    /// The description and labels of one PR, when the provider's list omits them.
    pub(super) fn spawn_load_info(&mut self, pr_id: u64) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::Info(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_info(pr_id),
            move |r| Read::Info(pr_id, r),
        );
    }

    pub(super) fn spawn_merge(&self, ticket: WriteTicket, strategy: MergeStrategy) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.merge(pr_id, strategy));
    }

    pub(super) fn spawn_decline(&self, ticket: WriteTicket) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.decline(pr_id));
    }

    pub(super) fn spawn_reopen(&self, ticket: WriteTicket) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.reopen(pr_id));
    }

    pub(super) fn spawn_comment(&self, ticket: WriteTicket, target: CommentTarget, text: String) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || match target {
            CommentTarget::Line(a) => provider.post_comment(pr_id, &a, &text),
            CommentTarget::Pr => provider.post_pr_comment(pr_id, &text),
            CommentTarget::Reply(parent) => provider.reply_comment(pr_id, parent, &text),
            CommentTarget::Edit { id, review } => provider.edit_comment(pr_id, id, review, &text),
            // Review verdicts are routed to `spawn_submit_full_review` upstream.
            CommentTarget::Review { .. } => Err(FetchError::InvalidInput(
                "A review verdict cannot be posted as a plain comment.".into(),
            )),
        });
    }

    pub(super) fn spawn_delete_comment(&self, ticket: WriteTicket, comment_id: u64, review: bool) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || {
            provider.delete_comment(pr_id, comment_id, review)
        });
    }

    /// Flush a whole review at once: the queued line `comments` plus the
    /// `verdict` and its summary `body`. GitHub sends one atomic call; Bitbucket
    /// posts the comments then flips status (see the provider impls).
    pub(super) fn spawn_submit_full_review(
        &self,
        ticket: WriteTicket,
        verdict: ReviewVerdict,
        body: String,
        user: String,
        comments: Vec<PendingComment>,
    ) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
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
        self.spawn_write(ticket, move || {
            provider
                .submit_full_review(pr_id, verdict, &body, &user, &review_comments)
                .map_err(|error| WriteError::from_review(error, body))
        });
    }

    pub(super) fn spawn_resolve_thread(
        &self,
        ticket: WriteTicket,
        node_id: Option<String>,
        comment_id: Option<u64>,
        resolved: bool,
    ) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || {
            provider.set_thread_resolved(pr_id, node_id.as_deref(), comment_id, resolved)
        });
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
        for group in PrGroup::ALL {
            if group == PrGroup::Open || self.state.store.group_loaded(group) {
                self.reload_resource(FetchKey::Prs(group));
            }
        }
        self.reload_resource(FetchKey::Mergeability(pr_id));
    }

    pub(super) fn load_resource(&mut self, key: FetchKey) {
        match key {
            FetchKey::Prs(group) => self.spawn_load_prs(group, None),
            FetchKey::Commits(id) => self.spawn_load_commits(id),
            FetchKey::Diff(id) => self.spawn_load_diff(id),
            FetchKey::Builds(id) => self.spawn_load_builds(id),
            FetchKey::Activity(id) => self.spawn_load_activity(id),
            FetchKey::Mergeability(id) => self.spawn_load_mergeability(id),
            FetchKey::Info(id) => self.spawn_load_info(id),
            FetchKey::CommitDiff(id, oid) => self.spawn_load_commit_diff(id, oid),
        }
    }

    /// Read a resource that has not been read yet; one already loaded or on
    /// its way is left alone.
    pub(super) fn ensure_loaded(&mut self, key: FetchKey) {
        if self.state.store.start_loading(&key) {
            self.load_resource(key);
        }
    }
}
