use super::{
    DetailContext, DetailView, PrDetailScreen,
    dialogs::{
        confirm::{ConfirmDialog, ConfirmKind},
        merge::MergeDialog,
        review::{ReviewContext, ReviewDialog},
    },
};
use crate::{
    app::{
        action::{Command, ConfirmAction, Effect, MergeAction, PrAction, ReviewAction},
        reviews::CommentTarget,
    },
    tui::{
        component::Component,
        components::comment_editor::{CommentDraft, CommentEditor},
    },
};

impl PrDetailScreen {
    /// Keep navigation and unfinished editors scoped to their PR for this session.
    pub fn open(&mut self, pr_id: u64) {
        if let Some(previous) = self.pr_id {
            self.navigation.insert(
                previous,
                super::DetailNavigation {
                    overview: std::mem::take(&mut self.overview),
                    builds: std::mem::take(&mut self.builds),
                    description: std::mem::take(&mut self.description),
                    diff: std::mem::take(&mut self.diff),
                    commits: std::mem::take(&mut self.commits),
                    tab: self.active_tab,
                },
            );
            self.editors
                .insert(previous, std::mem::take(&mut self.editor));
        }
        let editors = std::mem::take(&mut self.editors);
        let mut navigation = std::mem::take(&mut self.navigation);
        let position = navigation.remove(&pr_id).unwrap_or_default();
        *self = Self {
            pr_id: Some(pr_id),
            editors,
            navigation,
            active_tab: position.tab,
            overview: position.overview,
            builds: position.builds,
            description: position.description,
            diff: position.diff,
            commits: position.commits,
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

    pub(super) const fn view<'a>(&'a self, ctx: &DetailContext<'a>) -> DetailView<'a> {
        DetailView::new(self, ctx)
    }

    fn open_draft(&mut self, target: Option<CommentTarget>) {
        if self.editor.draft.is_some() {
            self.editor.resuming = true;
            self.editor.suspended = false;
            return;
        }
        if let Some(target) = target {
            self.editor = CommentEditor {
                draft: Some(CommentDraft {
                    target,
                    text: String::new(),
                }),
                ..CommentEditor::default()
            };
        }
    }

    const fn command(pr_id: u64, command: Command) -> Effect {
        Effect::Command { pr_id, command }
    }

    pub(super) fn dismiss_error(&mut self, pr_id: u64) -> Effect {
        self.error = super::dialogs::error::ErrorDialog::default();
        Effect::DismissError { pr_id }
    }

    pub(super) fn confirm_action(&mut self, action: ConfirmAction, pr_id: u64) -> Option<Effect> {
        match action {
            ConfirmAction::Move(_) => {
                self.confirm.as_mut()?.update(action, &());
                None
            }
            ConfirmAction::Close => {
                self.confirm = None;
                None
            }
            ConfirmAction::Accept => {
                let command = match self.confirm.take()?.accepted()? {
                    ConfirmKind::Decline => Command::Decline,
                    ConfirmKind::Reopen => Command::Reopen,
                    ConfirmKind::DiscardReview => Command::AbandonReview,
                    ConfirmKind::DeleteComment { id, review } => {
                        Command::DeleteComment { id, review }
                    }
                };
                Some(Self::command(pr_id, command))
            }
        }
    }

    pub(super) fn review_action(
        &mut self,
        action: ReviewAction,
        pr_id: u64,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        match action {
            ReviewAction::Move(_) | ReviewAction::Preview => {
                let review_ctx = ReviewContext {
                    options: self.view(ctx).review_context().options,
                    pending: ctx.store.reviews.get(&pr_id),
                };
                self.review_picker.as_mut()?.update(action, &review_ctx);
                None
            }
            ReviewAction::Close => {
                self.review_picker = None;
                None
            }
            ReviewAction::Select => {
                let verdict = self
                    .review_picker
                    .as_ref()?
                    .selected(&self.view(ctx).review_context())?;
                self.review_picker = None;
                if verdict.needs_body() {
                    self.open_draft(Some(CommentTarget::Review { verdict }));
                    return None;
                }
                Some(Self::command(
                    pr_id,
                    Command::SubmitReview {
                        verdict,
                        body: String::new(),
                    },
                ))
            }
        }
    }

    pub(super) fn merge_action(
        &mut self,
        action: MergeAction,
        pr_id: u64,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let strategies = ctx.store.capabilities.merge_strategies.as_slice();
        match action {
            MergeAction::Move(_) => {
                self.merge_picker.as_mut()?.update(action, &strategies);
                None
            }
            MergeAction::Close => {
                self.merge_picker = None;
                None
            }
            MergeAction::Select => {
                let strategy = self.merge_picker.take()?.selected(strategies)?;
                Some(Self::command(pr_id, Command::Merge(strategy)))
            }
        }
    }

    pub(super) fn submit_editor(&self, pr_id: u64) -> Option<Effect> {
        let draft = self.editor.draft.as_ref()?;
        if draft.text.trim().is_empty() {
            return None;
        }
        Some(Self::command(
            pr_id,
            Command::SubmitComment {
                target: draft.target.clone(),
                text: draft.text.clone(),
            },
        ))
    }

    pub(super) fn pr_action(
        &mut self,
        action: PrAction,
        pr_id: u64,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let command = match action {
            PrAction::OpenReviewPicker => {
                self.review_picker = Some(ReviewDialog::new(&self.view(ctx).review_context()));
                return None;
            }
            PrAction::FinishReview => {
                if ctx.store.reviews.contains_key(&pr_id) {
                    self.review_picker = Some(ReviewDialog::new(&self.view(ctx).review_context()));
                }
                return None;
            }
            PrAction::StartReview => Command::StartReview,
            PrAction::AbandonReview => {
                if ctx
                    .store
                    .reviews
                    .get(&pr_id)
                    .is_some_and(|review| !review.comments.is_empty())
                {
                    self.confirm = Some(ConfirmDialog::new(ConfirmKind::DiscardReview));
                    return None;
                }
                Command::AbandonReview
            }
            PrAction::RemovePendingComment => {
                Command::RemovePendingComment(self.surface(ctx.tab).diff_viewer()?.pane_pending?)
            }
            PrAction::OpenMergePicker => {
                if !ctx.store.capabilities.merge_strategies.is_empty() {
                    self.merge_picker = Some(MergeDialog::default());
                }
                return None;
            }
            PrAction::OpenDecline => {
                self.confirm = Some(ConfirmDialog::new(ConfirmKind::Decline));
                return None;
            }
            PrAction::OpenReopen => {
                self.confirm = Some(ConfirmDialog::new(ConfirmKind::Reopen));
                return None;
            }
            PrAction::OpenComment => {
                self.open_draft(self.view(ctx).comment_target());
                return None;
            }
            PrAction::OpenReply => {
                self.open_draft(self.view(ctx).reply_target());
                return None;
            }
            PrAction::EditComment => {
                let view = self.view(ctx);
                let selected = view.editable_selected()?;
                let id = selected.id?;
                let text = view.find_comment(id, selected.review)?.content.clone();
                if self.editor.draft.is_some() {
                    self.editor.resuming = true;
                    self.editor.suspended = false;
                    return None;
                }
                self.editor = CommentEditor::default();
                self.editor.draft = Some(CommentDraft {
                    target: CommentTarget::Edit {
                        id,
                        review: selected.review,
                    },
                    text,
                });
                return None;
            }
            PrAction::DeleteComment => {
                let selected = self.view(ctx).editable_selected()?;
                let comment = self.view(ctx).find_comment(selected.id?, selected.review)?;
                let preview = format!("@{}: {}", comment.author.username, comment.content);
                self.confirm = Some(
                    ConfirmDialog::new(ConfirmKind::DeleteComment {
                        id: selected.id?,
                        review: selected.review,
                    })
                    .with_context(preview),
                );
                return None;
            }
            PrAction::ResolveThread => {
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
        };
        Some(Self::command(pr_id, command))
    }
}

impl PrDetailScreen {
    pub fn draft_snapshot(&self) -> std::collections::BTreeMap<u64, CommentDraft> {
        let mut drafts: std::collections::BTreeMap<_, _> = self
            .editors
            .iter()
            .filter_map(|(id, editor)| editor.draft.clone().map(|draft| (*id, draft)))
            .collect();
        if let Some(id) = self.pr_id {
            drafts.remove(&id);
            if let Some(draft) = &self.editor.draft {
                drafts.insert(id, draft.clone());
            }
        }
        drafts
    }
    pub fn restore_drafts(&mut self, drafts: std::collections::BTreeMap<u64, CommentDraft>) {
        self.editors = drafts
            .into_iter()
            .map(|(id, draft)| {
                (
                    id,
                    CommentEditor {
                        draft: Some(draft),
                        suspended: true,
                        resuming: true,
                        ..CommentEditor::default()
                    },
                )
            })
            .collect();
    }
}
