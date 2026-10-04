use crate::{
    domain::{
        capabilities::{Capabilities, Feature},
        comment::{CommentKey, NonBlank, ThreadHandle},
        pr::{MergeStrategy, PrId},
        review::{CommentTarget, PendingComment, ReviewVerdict},
    },
    tui::app::{
        App,
        store::{Operation, WriteTicket},
    },
};

/// Fully resolved user intent; no dialog or editor state is read by the app.
#[derive(Debug)]
pub enum Command {
    StartReview,
    AbandonReview,
    RemovePendingComment(usize),
    SubmitComment {
        target: CommentTarget,
        text: NonBlank,
    },
    SubmitReview {
        verdict: ReviewVerdict,
        body: String,
    },
    Merge(MergeStrategy),
    RerunFailedBuilds,
    Decline,
    Reopen,
    DeleteComment(CommentKey),
    ResolveThread {
        thread: ThreadHandle,
        resolved: bool,
    },
}

impl Command {
    pub fn supported_by(&self, caps: &Capabilities) -> bool {
        match self {
            Self::StartReview | Self::AbandonReview | Self::RemovePendingComment(_) => {
                caps.reviews()
            }
            Self::SubmitReview { verdict, .. } => caps.can_submit_verdict(*verdict, false),
            Self::SubmitComment { target, .. } => match target {
                CommentTarget::Pr => caps.supports(Feature::PrComments),
                CommentTarget::Line(_) => caps.supports(Feature::InlineComments),
                CommentTarget::Reply(_) => caps.supports(Feature::Replies),
                CommentTarget::Edit(_) => caps.supports(Feature::EditComments),
                CommentTarget::Review { verdict } => caps.can_submit_verdict(*verdict, false),
            },
            Self::Merge(strategy) => caps.merge_strategies.contains(strategy),
            Self::RerunFailedBuilds => caps.supports(Feature::RerunBuilds),
            Self::Decline => caps.supports(Feature::ClosePr),
            Self::Reopen => caps.supports(Feature::ReopenPr),
            Self::DeleteComment { .. } => caps.supports(Feature::DeleteComments),
            Self::ResolveThread { .. } => caps.supports(Feature::ResolveThreads),
        }
    }
}

impl App {
    pub(super) fn execute(&mut self, pr_id: PrId, command: Command) {
        if !command.supported_by(&self.state.store.capabilities) {
            self.state.store.errors.insert(
                pr_id,
                "This action is not supported by the connected provider.".into(),
            );
            return;
        }
        // One mutation per PR: payloads and review queues remain stable until completion.
        if self.state.store.operations.contains_key(&pr_id) {
            return;
        }
        match command {
            Command::StartReview => {
                self.state.store.reviews.entry(pr_id).or_default();
            }
            Command::AbandonReview => {
                self.state.store.reviews.remove(&pr_id);
            }
            Command::RemovePendingComment(index) => {
                if let Some(review) = self.state.store.reviews.get_mut(&pr_id)
                    && index < review.comments.len()
                {
                    review.comments.remove(index);
                }
            }
            Command::SubmitComment { target, text } => {
                let text = text.into_string();
                if let CommentTarget::Line(anchor) = &target
                    && let Some(review) = self.state.store.reviews.get_mut(&pr_id)
                {
                    review.comments.push(PendingComment {
                        anchor: anchor.clone(),
                        text,
                    });
                    self.state.ui.detail.submission_finished(pr_id, true);
                    return;
                }
                if let CommentTarget::Review { verdict } = target {
                    self.submit_review_verdict(pr_id, verdict, text);
                } else if let Some(ticket) = self.begin_write(pr_id, Operation::Comment) {
                    self.spawn_comment(ticket, target, text);
                }
            }
            Command::SubmitReview { verdict, body } => {
                self.submit_review_verdict(pr_id, verdict, body);
            }
            Command::Merge(strategy) => {
                if let Some(ticket) = self.begin_write(pr_id, Operation::Merge) {
                    self.spawn_merge(ticket, strategy);
                }
            }
            Command::RerunFailedBuilds => {
                if let Some(ticket) = self.begin_write(pr_id, Operation::RerunBuilds) {
                    self.spawn_rerun_builds(ticket);
                }
            }
            Command::Decline => {
                if let Some(ticket) = self.begin_write(pr_id, Operation::Decline) {
                    self.spawn_decline(ticket);
                }
            }
            Command::Reopen => {
                if let Some(ticket) = self.begin_write(pr_id, Operation::Reopen) {
                    self.spawn_reopen(ticket);
                }
            }
            Command::DeleteComment(comment) => {
                if let Some(ticket) = self.begin_write(pr_id, Operation::Moderation) {
                    self.spawn_delete_comment(ticket, comment);
                }
            }
            Command::ResolveThread { thread, resolved } => {
                if let Some(ticket) = self.begin_write(pr_id, Operation::Moderation) {
                    self.spawn_resolve_thread(ticket, thread, resolved);
                }
            }
        }
    }

    /// The step every write goes through: record the operation, then journal
    /// the drafts so an interrupted request is known about after a restart.
    /// `None` when another write is pending for the PR, or the journal could
    /// not be written; nothing is sent then.
    fn begin_write(&mut self, pr_id: PrId, operation: Operation) -> Option<WriteTicket> {
        let ticket = self.state.store.begin_write(pr_id, operation)?;
        self.checkpoint_submission(pr_id).then_some(ticket)
    }

    fn submit_review_verdict(&mut self, pr_id: PrId, verdict: ReviewVerdict, body: String) {
        let own_pr = self.state.store.cache.prs.loaded().is_some_and(|prs| {
            prs.iter()
                .any(|pr| pr.id == pr_id && self.state.store.current_user.is(&pr.author.username))
        });
        if !self
            .state
            .store
            .capabilities
            .can_submit_verdict(verdict, own_pr)
        {
            self.state.store.errors.insert(
                pr_id,
                "This review verdict is unavailable for this PR.".into(),
            );
            return;
        }
        let one_revision = self
            .state
            .store
            .capabilities
            .review
            .as_ref()
            .is_some_and(|caps| {
                caps.submission
                    == crate::domain::capabilities::ReviewSubmission::AtomicSingleRevision
            });
        if one_revision
            && let Some(review) = self.state.store.reviews.get(&pr_id)
            && let Some(first) = review.comments.first()
            && review
                .comments
                .iter()
                .any(|c| c.anchor.revision != first.anchor.revision)
        {
            self.state.store.errors.insert(pr_id, "This provider requires one diff revision per review. Submit different revisions separately.".into());
            return;
        }
        let user = self.state.store.current_user.as_str().to_owned();
        let review = self.state.store.reviews.entry(pr_id).or_default();
        let body = if review.submitted_summary.as_ref() == Some(&body) {
            String::new()
        } else {
            body
        };
        let comments = review.comments.clone();
        if let Some(ticket) = self.begin_write(pr_id, Operation::Review) {
            self.spawn_submit_full_review(ticket, verdict, body, user, comments);
        }
    }
}
