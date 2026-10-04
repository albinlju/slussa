use super::{
    DetailContext, DetailView, Overlay, PrDetailScreen,
    dialogs::{
        confirm::{ConfirmDialog, ConfirmKind},
        merge::MergeDialog,
        review::{ReviewContext, ReviewDialog},
    },
};
use crate::{
    domain::{pr::PrId, review::CommentTarget},
    tui::{
        app::{commands::Command, effect::Effect},
        ui::{
            action::{ConfirmAction, MergeAction, PrAction, ReviewAction},
            component::Component,
            components::comment_editor::{CommentDraft, CommentEditor},
        },
    },
};

impl PrDetailScreen {
    /// Keep navigation and unfinished editors scoped to their PR for this session.
    pub fn open(&mut self, pr_id: PrId) {
        if let Some(previous) = self.pr_id {
            self.navigation.insert(
                previous,
                super::screen::DetailNavigation {
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
        let mut editors = editors;
        let editor = editors.remove(&pr_id).unwrap_or_default();
        // Every field is named: one added later has to say what opening a PR
        // does to it, instead of being reset by a `..Self::default()`.
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
            editor,
            overlay: None,
            error: super::dialogs::error::ErrorDialog::default(),
        };
    }

    /// An acknowledgement affects only the editor that submitted the payload.
    pub fn submission_finished(&mut self, pr_id: PrId, success: bool) {
        let editor = if self.pr_id == Some(pr_id) {
            Some(&mut self.editor)
        } else {
            self.editors.get_mut(&pr_id)
        };
        if success && let Some(editor) = editor {
            editor.clear();
        }
    }

    pub(super) const fn view<'a>(&'a self, ctx: &DetailContext<'a>) -> DetailView<'a> {
        DetailView::new(self, ctx)
    }

    fn open_draft(&mut self, target: Option<CommentTarget>) {
        if self.editor.resume() {
            return;
        }
        if let Some(target) = target {
            self.editor = CommentEditor::start(target, String::new());
        }
    }

    fn ask(&mut self, dialog: ConfirmDialog) {
        self.overlay = Some(Overlay::Confirm(dialog));
    }

    /// Close the dialog if it is the one the message came from.
    fn close(&mut self, is_this: impl FnOnce(&Overlay) -> bool) {
        if self.overlay.as_ref().is_some_and(is_this) {
            self.overlay = None;
        }
    }

    const fn command(pr_id: PrId, command: Command) -> Effect {
        Effect::Command { pr_id, command }
    }

    pub(super) fn dismiss_error(&mut self, pr_id: PrId) -> Effect {
        self.error = super::dialogs::error::ErrorDialog::default();
        Effect::DismissError { pr_id }
    }

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
                    },
                ))
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
            MergeAction::Select => {
                let strategy = self.merge_picker()?.selected(strategies);
                self.overlay = None;
                let strategy = strategy?;
                Some(Self::command(pr_id, Command::Merge(strategy)))
            }
        }
    }

    pub(super) fn submit_editor(&self, pr_id: PrId) -> Option<Effect> {
        let CommentDraft { target, text } = self.editor.draft()?;
        // A blank draft is not a comment: Ctrl+S does nothing until it has text.
        let text = crate::domain::comment::NonBlank::new(text)?;
        Some(Self::command(
            pr_id,
            Command::SubmitComment { target, text },
        ))
    }

    pub(super) fn pr_action(
        &mut self,
        action: PrAction,
        pr_id: PrId,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let command = match action {
            PrAction::OpenReviewPicker => {
                let dialog = ReviewDialog::new(&self.view(ctx).review_context());
                self.overlay = Some(Overlay::Review(dialog));
                return None;
            }
            PrAction::FinishReview => {
                if ctx.store.reviews.contains_key(&pr_id) {
                    let dialog = ReviewDialog::new(&self.view(ctx).review_context());
                    self.overlay = Some(Overlay::Review(dialog));
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
                    self.ask(ConfirmDialog::new(ConfirmKind::DiscardReview));
                    return None;
                }
                Command::AbandonReview
            }
            PrAction::RemovePendingComment => Command::RemovePendingComment(
                self.surface(ctx.tab).diff_viewer()?.focused_pending()?,
            ),
            PrAction::OpenMergePicker => {
                if !ctx.store.capabilities.merge_strategies.is_empty() {
                    self.overlay = Some(Overlay::Merge(MergeDialog::default()));
                }
                return None;
            }
            PrAction::OpenDecline => {
                self.ask(ConfirmDialog::new(ConfirmKind::Decline));
                return None;
            }
            PrAction::OpenReopen => {
                self.ask(ConfirmDialog::new(ConfirmKind::Reopen));
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
                let comment = view.editable_selected()?;
                let text = view.find_comment(comment)?.content.clone();
                if !self.editor.resume() {
                    self.editor = CommentEditor::start(CommentTarget::Edit(comment), text);
                }
                return None;
            }
            PrAction::DeleteComment => {
                let key = self.view(ctx).editable_selected()?;
                let comment = self.view(ctx).find_comment(key)?;
                let preview = format!("@{}: {}", comment.author.username, comment.content);
                self.ask(ConfirmDialog::new(ConfirmKind::DeleteComment(key)).with_preview(preview));
                return None;
            }
            PrAction::RerunBuilds => Command::RerunFailedBuilds,
            PrAction::ResolveThread => {
                let thread = self.view(ctx).focused_thread()?;
                Command::ResolveThread {
                    thread: thread.handle.clone()?,
                    resolved: !thread.resolved,
                }
            }
        };
        Some(Self::command(pr_id, command))
    }
}

impl PrDetailScreen {
    pub fn draft_snapshot(&self) -> std::collections::BTreeMap<PrId, CommentDraft> {
        let mut drafts: std::collections::BTreeMap<_, _> = self
            .editors
            .iter()
            .filter_map(|(id, editor)| editor.draft().map(|draft| (*id, draft)))
            .collect();
        if let Some(id) = self.pr_id {
            drafts.remove(&id);
            if let Some(draft) = self.editor.draft() {
                drafts.insert(id, draft);
            }
        }
        drafts
    }
    pub fn restore_drafts(&mut self, drafts: std::collections::BTreeMap<PrId, CommentDraft>) {
        self.editors = drafts
            .into_iter()
            .map(|(id, draft)| (id, CommentEditor::restored(draft)))
            .collect();
    }
}
