//! Work that leaves the UI thread. `spawn_fetch` is the one way out; a read
//! goes through `spawn_read` with the `FetchTicket` that registered it, a
//! write through `spawn_write` with its `WriteTicket`.
use crate::{
    domain::{
        ci::JobId,
        comment::{CommentKey, ThreadHandle},
        commit::CommitOid,
        pr::{AutoMerge, DeletableBranch, MergeStrategy, PrGroup, PrId},
        review::{
            CommentAnchor, CommentTarget, PendingComment, Rerequest, ReviewComment, ReviewVerdict,
            ReviewedHead,
        },
    },
    providers::FetchError,
    tui::app::{
        App,
        effect::{Read, TaskResult, WriteError},
        store::{FetchKey, FetchTicket, PrResource, WriteTicket},
    },
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

    /// Read one PR by its number, for the reader to open: it may not be in the list.
    pub(super) fn spawn_load_pr(&mut self, pr_id: PrId) {
        let Some(ticket) = self.state.store.begin_fetch(FetchKey::One(pr_id)) else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_pr(pr_id),
            move |result| Read::Pr(pr_id, result),
        );
    }

    pub(super) fn spawn_load_commits(&mut self, pr_id: PrId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::Commits, pr_id))
        else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_commits(pr_id),
            move |r| Read::Commits(pr_id, r),
        );
    }

    pub(super) fn spawn_load_diff(&mut self, pr_id: PrId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::Diff, pr_id))
        else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_diff(pr_id),
            move |r| Read::Diff(pr_id, r),
        );
    }

    pub(super) fn spawn_load_builds(&mut self, pr_id: PrId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::Builds, pr_id))
        else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_builds(pr_id),
            move |r| Read::Builds(pr_id, r),
        );
    }

    pub(super) fn spawn_load_commit_diff(&mut self, pr_id: PrId, oid: CommitOid) {
        let key = FetchKey::Pr(PrResource::CommitDiff(oid.clone()), pr_id);
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

    pub(super) fn spawn_load_build_log(&mut self, pr_id: PrId, job: JobId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::BuildLog(job), pr_id))
        else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_build_log(job),
            move |r| Read::BuildLog(pr_id, job, r),
        );
    }

    pub(super) fn spawn_load_activity(&mut self, pr_id: PrId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::Activity, pr_id))
        else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_activity(pr_id),
            move |r| Read::Activity(pr_id, r),
        );
    }

    pub(super) fn spawn_load_mergeability(&mut self, pr_id: PrId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::Mergeability, pr_id))
        else {
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
    pub(super) fn spawn_load_info(&mut self, pr_id: PrId) {
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::Info, pr_id))
        else {
            return;
        };
        let provider = self.provider.clone();
        self.spawn_read(
            ticket,
            move || provider.fetch_info(pr_id),
            move |r| Read::Info(pr_id, r),
        );
    }

    pub(super) fn spawn_merge(
        &self,
        ticket: WriteTicket,
        strategy: MergeStrategy,
        delete: Option<DeletableBranch>,
        head: ReviewedHead,
    ) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || {
            provider.merge(pr_id, strategy, delete.as_ref(), &head)
        });
    }

    pub(super) fn spawn_auto_merge(&self, ticket: WriteTicket, change: AutoMerge) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.set_auto_merge(pr_id, &change));
    }

    pub(super) fn spawn_rerun_builds(&self, ticket: WriteTicket) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.rerun_failed_builds(pr_id));
    }

    pub(super) fn spawn_rerequest_review(&self, ticket: WriteTicket, who: Rerequest) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.rerequest_review(pr_id, &who));
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
            CommentTarget::Line(anchor) => provider.post_comment(
                pr_id,
                &postable(
                    anchor,
                    text,
                    "Reload the diff before commenting: its revision is unknown.",
                )?,
            ),
            CommentTarget::Pr => provider.post_pr_comment(pr_id, &text),
            CommentTarget::Reply(parent) => provider.reply_comment(pr_id, parent, &text),
            CommentTarget::Edit(comment) => provider.edit_comment(pr_id, comment, &text),
            // Review verdicts are routed to `spawn_submit_full_review` upstream.
            CommentTarget::Review { .. } => Err(FetchError::InvalidInput(
                "A review verdict cannot be posted as a plain comment.".into(),
            )),
        });
    }

    pub(super) fn spawn_delete_comment(&self, ticket: WriteTicket, comment: CommentKey) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || provider.delete_comment(pr_id, comment));
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
        head: ReviewedHead,
    ) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || {
            let hint = "Reload the diff and recreate comments with an unknown revision.";
            let comments = comments
                .into_iter()
                .map(|comment| postable(comment.anchor, comment.text, hint))
                .collect::<Result<Vec<_>, _>>()?;
            provider
                .submit_full_review(pr_id, verdict, &body, &user, &comments, &head)
                .map_err(|error| WriteError::from_review(error, body))
        });
    }

    pub(super) fn spawn_resolve_thread(
        &self,
        ticket: WriteTicket,
        thread: ThreadHandle,
        resolved: bool,
    ) {
        let provider = self.provider.clone();
        let pr_id = ticket.pr_id();
        self.spawn_write(ticket, move || {
            provider.set_thread_resolved(pr_id, &thread, resolved)
        });
    }
}

/// The one place an anchor becomes something a provider can post. Nothing is
/// sent for a comment whose diff revision is unknown; `hint` says what to do.
fn postable(anchor: CommentAnchor, body: String, hint: &str) -> Result<ReviewComment, FetchError> {
    ReviewComment::new(anchor, body).ok_or_else(|| FetchError::InvalidInput(hint.into()))
}

impl App {
    /// A mutation must be followed by a fetch started after its acknowledgement.
    /// Read the PR's builds again: they were just started over.
    pub(super) fn reload_builds(&mut self, pr_id: PrId) {
        self.reload_resource(FetchKey::Pr(PrResource::Builds, pr_id));
    }

    fn reload_resource(&mut self, key: FetchKey) {
        if self.state.store.fetches.contains(&key) {
            self.state.store.reload_after_fetch.insert(key);
        } else {
            self.load_resource(key);
        }
    }

    pub(super) fn reload_after_mutation(&mut self, pr_id: PrId) {
        self.reload_resource(FetchKey::Pr(PrResource::Activity, pr_id));
        for group in PrGroup::ALL {
            if group == PrGroup::Open || self.state.store.group_loaded(group) {
                self.reload_resource(FetchKey::Prs(group));
            }
        }
        self.reload_resource(FetchKey::Pr(PrResource::Mergeability, pr_id));
    }

    pub(super) fn load_resource(&mut self, key: FetchKey) {
        match key {
            FetchKey::Prs(group) => self.spawn_load_prs(group, None),
            FetchKey::One(id) => self.spawn_load_pr(id),
            FetchKey::Pr(PrResource::Commits, id) => self.spawn_load_commits(id),
            FetchKey::Pr(PrResource::Diff, id) => self.spawn_load_diff(id),
            FetchKey::Pr(PrResource::Builds, id) => self.spawn_load_builds(id),
            FetchKey::Pr(PrResource::Activity, id) => self.spawn_load_activity(id),
            FetchKey::Pr(PrResource::Mergeability, id) => self.spawn_load_mergeability(id),
            FetchKey::Pr(PrResource::Info, id) => self.spawn_load_info(id),
            FetchKey::Pr(PrResource::CommitDiff(oid), id) => self.spawn_load_commit_diff(id, oid),
            FetchKey::Pr(PrResource::BuildLog(job), id) => self.spawn_load_build_log(id, job),
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
