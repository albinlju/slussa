use super::{
    DetailContext, DetailView, PrDetailScreen,
    dialogs::{
        confirm::{ConfirmDialog, ConfirmKind},
        review::ReviewDialog,
    },
};
use crate::{
    app::{
        action::{Action, Command, DetailAction},
        navigation::Screen,
        reviews::CommentTarget,
    },
    tui::components::comment_editor::{CommentDraft, CommentEditor},
};

impl PrDetailScreen {
    /// Reset view navigation while retaining each PR's unfinished editor.
    pub fn open(&mut self, pr_id: u64) {
        if let Some(previous) = self.pr_id {
            self.editors
                .insert(previous, std::mem::take(&mut self.editor));
        }
        let editors = std::mem::take(&mut self.editors);
        *self = Self {
            pr_id: Some(pr_id),
            editors,
            ..Self::default()
        };
        self.editor = self.editors.remove(&pr_id).unwrap_or_default();
    }

    /// An acknowledgement affects only the editor that submitted the payload.
    pub fn submission_finished(&mut self, pr_id: u64, success: bool) {
        let editor = if self.pr_id == Some(pr_id) {
            Some(&mut self.editor)
        } else {
            self.editors.get_mut(&pr_id)
        };
        if success && let Some(editor) = editor {
            editor.draft = None;
        }
    }

    fn view<'a>(&'a self, ctx: &'a DetailContext<'a>) -> DetailView<'a> {
        DetailView {
            detail: self,
            store: ctx.store,
            screen: ctx.screen,
            refreshing: ctx.refreshing,
        }
    }

    fn open_draft(&mut self, target: Option<CommentTarget>) {
        if let Some(target) = target {
            self.editor = CommentEditor {
                draft: Some(CommentDraft {
                    target,
                    text: String::new(),
                }),
            };
        }
    }

    pub(super) fn interaction(
        &mut self,
        action: DetailAction,
        ctx: &DetailContext<'_>,
    ) -> Option<Action> {
        let Screen::Detail { pr_id, .. } = ctx.screen else {
            return None;
        };
        let command = match action {
            DetailAction::OpenReviewPicker => {
                self.review_picker = Some(ReviewDialog::new(&self.view(ctx).review_context()));
                return None;
            }
            DetailAction::FinishReview => {
                if ctx.store.reviews.contains_key(&pr_id) {
                    self.review_picker = Some(ReviewDialog::new(&self.view(ctx).review_context()));
                }
                return None;
            }
            DetailAction::ReviewSelect => {
                let verdict = self
                    .review_picker
                    .as_ref()?
                    .selected(&self.view(ctx).review_context())?;
                self.review_picker = None;
                if verdict.needs_body() {
                    self.open_draft(Some(CommentTarget::Review { verdict }));
                    return None;
                }
                Command::SubmitReview {
                    verdict,
                    body: String::new(),
                }
            }
            DetailAction::StartReview => Command::StartReview,
            DetailAction::AbandonReview => Command::AbandonReview,
            DetailAction::RemovePendingComment => {
                Command::RemovePendingComment(self.active_diff_view().pane_pending?)
            }
            DetailAction::OpenComment => {
                self.open_draft(self.view(ctx).comment_target());
                return None;
            }
            DetailAction::OpenReply => {
                self.open_draft(self.view(ctx).reply_target());
                return None;
            }
            DetailAction::EditComment => {
                let view = self.view(ctx);
                let selected = view.editable_selected()?;
                let id = selected.id?;
                let text = view.find_comment(id)?.content.clone();
                self.editor.draft = Some(CommentDraft {
                    target: CommentTarget::Edit {
                        id,
                        review: selected.review,
                    },
                    text,
                });
                return None;
            }
            DetailAction::DeleteComment => {
                let selected = self.view(ctx).editable_selected()?;
                self.confirm = Some(ConfirmDialog::new(ConfirmKind::DeleteComment {
                    id: selected.id?,
                    review: selected.review,
                }));
                return None;
            }
            DetailAction::ResolveThread => {
                let thread = self.view(ctx).focused_thread()?;
                if thread.node_id.is_none() && thread.comment_id.is_none() {
                    return None;
                }
                Command::ResolveThread {
                    node_id: thread.node_id.clone(),
                    comment_id: thread.comment_id,
                    resolved: !thread.resolved,
                }
            }
            DetailAction::MergeSelect => Command::Merge(
                self.merge_picker
                    .take()?
                    .selected(&ctx.store.merge_strategies)?,
            ),
            DetailAction::SubmitConfirm => match self.confirm.take()?.accepted()? {
                ConfirmKind::Decline => Command::Decline,
                ConfirmKind::DeleteComment { id, review } => Command::DeleteComment { id, review },
            },
            DetailAction::CommentSubmit => {
                let draft = self.editor.draft.as_ref()?;
                if draft.text.trim().is_empty() {
                    return None;
                }
                Command::SubmitComment {
                    target: draft.target.clone(),
                    text: draft.text.clone(),
                }
            }
            DetailAction::DismissError => Command::DismissError,
            _ => unreachable!("local interaction handled by screen"),
        };
        Some(Action::Command { pr_id, command })
    }
}
