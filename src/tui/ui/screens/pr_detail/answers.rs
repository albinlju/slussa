//! What the reader's answer in a dialog does: a confirmation, the review
//! dialog, the list of issues and the merge dialog, each resolved to the command
//! it stands for or to the next thing to ask.

use super::{
    DetailContext, Overlay, PrDetailScreen,
    dialogs::{confirm::ConfirmKind, merge::AutoMergeOffer, review::ReviewContext},
    interactions::head_unknown,
    view::issues_to_open,
};
use crate::{
    domain::{
        pr::{AutoMerge, PrId},
        review::CommentTarget,
    },
    tui::{
        app::{commands::Command, effect::Effect},
        ui::{
            action::{ConfirmAction, IssueAction, MergeAction, ReviewAction},
            component::Component,
        },
    },
};

impl PrDetailScreen {
    pub(super) fn confirm_action(&mut self, action: ConfirmAction, pr_id: PrId) -> Option<Effect> {
        match action {
            ConfirmAction::Move(_) => {
                if let Some(Overlay::Confirm(dialog)) = &mut self.overlay {
                    dialog.update(action, &());
                }
                None
            }
            ConfirmAction::Close => {
                self.close(|overlay| matches!(overlay, Overlay::Confirm(_)));
                None
            }
            ConfirmAction::Accept => {
                let accepted = self.confirm()?.accepted();
                self.overlay = None;
                let command = match accepted? {
                    ConfirmKind::AgentReview => return Some(Effect::RunAgentReview { pr_id }),
                    ConfirmKind::StopAgentReview => {
                        return Some(Effect::StopAgentReview { pr_id });
                    }
                    ConfirmKind::Decline => Command::Decline,
                    ConfirmKind::Reopen => Command::Reopen,
                    ConfirmKind::DiscardReview => Command::AbandonReview,
                    ConfirmKind::DeleteComment(comment) => Command::DeleteComment(comment),
                };
                Some(Self::command(pr_id, command))
            }
        }
    }

    pub(super) fn review_action(
        &mut self,
        action: ReviewAction,
        pr_id: PrId,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        match action {
            ReviewAction::Move(_) | ReviewAction::Preview => {
                let review_ctx = ReviewContext {
                    options: self.view(ctx).review_context().options,
                    pending: ctx.store.reviews.get(&pr_id),
                };
                if let Some(Overlay::Review(dialog)) = &mut self.overlay {
                    dialog.update(action, &review_ctx);
                }
                None
            }
            ReviewAction::Close => {
                self.close(|overlay| matches!(overlay, Overlay::Review(_)));
                None
            }
            ReviewAction::Select => {
                let verdict = self
                    .review_picker()?
                    .selected(&self.view(ctx).review_context())?;
                let Some(head) = ctx.reviewed_head() else {
                    return Some(head_unknown(pr_id));
                };
                self.overlay = None;
                if verdict.needs_body() {
                    self.open_draft(Some(CommentTarget::Review { verdict }));
                    return None;
                }
                Some(Self::command(
                    pr_id,
                    Command::SubmitReview {
                        verdict,
                        body: String::new(),
                        head,
                    },
                ))
            }
        }
    }

    pub(super) fn issue_action(
        &mut self,
        action: IssueAction,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let issues = issues_to_open(ctx.data);
        match action {
            IssueAction::Move(_) => {
                if let Some(Overlay::Issues(dialog)) = &mut self.overlay {
                    dialog.update(action, &issues.len());
                }
                None
            }
            IssueAction::Close => {
                self.close(|overlay| matches!(overlay, Overlay::Issues(_)));
                None
            }
            IssueAction::Select => {
                let (issue, url) = *self.issue_picker()?.selected(&issues)?;
                self.overlay = None;
                Some(Effect::IssueLink {
                    number: issue.number,
                    url: url.to_owned(),
                })
            }
        }
    }

    pub(super) fn merge_action(
        &mut self,
        action: MergeAction,
        pr_id: PrId,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let strategies = ctx.store.capabilities.merge_strategies.as_slice();
        match action {
            MergeAction::Move(_) => {
                if let Some(Overlay::Merge(dialog)) = &mut self.overlay {
                    dialog.update(action, &strategies);
                }
                None
            }
            MergeAction::Close => {
                self.close(|overlay| matches!(overlay, Overlay::Merge(_)));
                None
            }
            MergeAction::Auto => {
                match AutoMergeOffer::of(&ctx.store.capabilities, ctx.mergeability()) {
                    AutoMergeOffer::Unavailable => {}
                    AutoMergeOffer::Available => {
                        if let Some(Overlay::Merge(dialog)) = &mut self.overlay {
                            dialog.when_ready = !dialog.when_ready;
                        }
                    }
                    AutoMergeOffer::On(_) => {
                        self.overlay = None;
                        return Some(Self::command(pr_id, Command::AutoMerge(AutoMerge::Off)));
                    }
                }
                None
            }
            MergeAction::DeleteBranch => {
                // Merging when ready leaves the branch to the repository, and the
                // dialog hides the box then: the choice cannot change unseen.
                let waiting = AutoMergeOffer::of(&ctx.store.capabilities, ctx.mergeability())
                    == AutoMergeOffer::Available;
                if ctx.deletable_branch().is_some()
                    && let Some(Overlay::Merge(dialog)) = &mut self.overlay
                    && !(dialog.when_ready && waiting)
                {
                    dialog.delete_branch = !dialog.delete_branch;
                }
                None
            }
            MergeAction::Select => {
                let dialog = self.merge_picker()?;
                let strategy = dialog.selected(strategies);
                let offered = AutoMergeOffer::of(&ctx.store.capabilities, ctx.mergeability())
                    == AutoMergeOffer::Available;
                if dialog.when_ready && !offered {
                    // What the PR waits on changed while the dialog was open: the
                    // dialog says merge now again, and asks for another Enter.
                    if let Some(Overlay::Merge(dialog)) = &mut self.overlay {
                        dialog.when_ready = false;
                    }
                    return None;
                }
                let when_ready = dialog.when_ready;
                let delete = ctx.deletable_branch().filter(|_| dialog.delete_branch);
                // What the reader has seen of the PR now, not when the dialog
                // opened: it is that commit the merge is tied to.
                let Some(head) = ctx.reviewed_head() else {
                    return Some(head_unknown(pr_id));
                };
                self.overlay = None;
                let strategy = strategy?;
                Some(Self::command(
                    pr_id,
                    if when_ready {
                        Command::AutoMerge(AutoMerge::On { strategy, head })
                    } else {
                        Command::Merge {
                            strategy,
                            delete,
                            head,
                        }
                    },
                ))
            }
        }
    }
}
