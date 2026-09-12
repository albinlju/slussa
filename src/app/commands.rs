use crate::app::{
    App,
    action::Command,
    reviews::{CommentTarget, PendingComment},
    store::Operation,
};

impl App {
    pub(super) fn execute(&mut self, pr_id: u64, command: Command) {
        if !command.supported_by(&self.state.store.capabilities) {
            self.state.store.errors.insert(
                pr_id,
                "This action is not supported by the connected provider.".into(),
            );
            return;
        }
        if matches!(command, Command::DismissError) {
            self.state.store.errors.remove(&pr_id);
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
                if text.trim().is_empty() {
                    return;
                }
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
                } else {
                    self.state
                        .store
                        .operations
                        .insert(pr_id, Operation::Comment);
                    self.spawn_comment(pr_id, target, text);
                }
            }
            Command::SubmitReview { verdict, body } => {
                self.submit_review_verdict(pr_id, verdict, body);
            }
            Command::Merge(strategy) => {
                self.state.store.operations.insert(pr_id, Operation::Merge);
                self.spawn_merge(pr_id, strategy);
            }
            Command::Decline => {
                self.state
                    .store
                    .operations
                    .insert(pr_id, Operation::Decline);
                self.spawn_decline(pr_id);
            }
            Command::DeleteComment { id, review } => {
                self.state
                    .store
                    .operations
                    .insert(pr_id, Operation::Moderation);
                self.spawn_delete_comment(pr_id, id, review);
            }
            Command::ResolveThread {
                node_id,
                comment_id,
                resolved,
            } => {
                self.state
                    .store
                    .operations
                    .insert(pr_id, Operation::Moderation);
                self.spawn_resolve_thread(pr_id, node_id, comment_id, resolved);
            }
            Command::DismissError => unreachable!(),
        }
    }

    fn submit_review_verdict(
        &mut self,
        pr_id: u64,
        verdict: crate::domain::review::ReviewVerdict,
        body: String,
    ) {
        let own_pr = match &self.state.store.cache.prs {
            crate::app::store::LoadState::Loaded(prs) => prs
                .iter()
                .any(|pr| pr.id == pr_id && pr.author.username == self.state.store.current_user),
            _ => false,
        };
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
        if self.state.store.capabilities.review_submission
            == crate::domain::capabilities::ReviewSubmission::AtomicSingleRevision
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
        self.state.store.operations.insert(pr_id, Operation::Review);
        let user = self.state.store.current_user.clone();
        let review = self.state.store.reviews.entry(pr_id).or_default();
        let body = if review.submitted_summary.as_ref() == Some(&body) {
            String::new()
        } else {
            body
        };
        let comments = review.comments.clone();
        self.spawn_submit_full_review(pr_id, verdict, body, user, comments);
    }
}
