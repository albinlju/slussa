use super::{
    DetailContext, DetailView, Overlay, PrDetailScreen,
    dialogs::{
        confirm::{ConfirmDialog, ConfirmKind},
        issues::IssueDialog,
        merge::MergeDialog,
        review::ReviewDialog,
    },
    view::issues_to_open,
};
use crate::{
    domain::{pr::PrId, review::CommentTarget, seen::How},
    tui::{
        app::{commands::Command, effect::Effect},
        ui::{
            action::PrAction,
            components::comment_editor::{CommentDraft, CommentEditor},
            components::diff_viewer::DiffViewer,
        },
    },
};

/// What is shown when a merge or a verdict is asked for and no commit is known
/// to tie it to. The keys are dimmed then, so this is for what changed since.
pub(super) fn head_unknown(pr_id: PrId) -> Effect {
    Effect::Report {
        pr_id,
        message: "The commit you are reviewing is not known. Refresh and try again.".into(),
    }
}

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
            since: None,
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

    pub(super) fn open_draft(&mut self, target: Option<CommentTarget>) {
        if self.editor.resume() {
            return;
        }
        if let Some(target) = target {
            self.editor = CommentEditor::start(target, String::new());
        }
    }

    /// `c`: a comment on what the cursor is on. On an agent's proposal it starts
    /// from the proposal's words, to edit and send as the reader's own, and the
    /// proposal is taken: it is not shown again, and the draft is what is kept.
    fn open_comment(&mut self, pr_id: PrId, ctx: &DetailContext<'_>) -> Option<Effect> {
        if self.editor.resume() {
            return None;
        }
        let target = self.view(ctx).comment_target()?;
        let proposed = self
            .surface(ctx.tab)
            .diff_viewer()
            .and_then(DiffViewer::focused_proposal)
            .and_then(|index| {
                let held = ctx.store.proposals.for_pr(pr_id)?;
                Some((index, held.comments.get(index)?.body().to_owned()))
            });
        if let Some((index, text)) = proposed {
            self.editor = CommentEditor::start(target, text);
            return Some(Effect::HandleProposal {
                pr_id,
                index,
                how: How::Taken,
            });
        }
        self.editor = CommentEditor::start(target, String::new());
        None
    }

    pub(super) fn ask(&mut self, dialog: ConfirmDialog) {
        self.overlay = Some(Overlay::Confirm(dialog));
    }

    /// Close the dialog if it is the one the message came from.
    pub(super) fn close(&mut self, is_this: impl FnOnce(&Overlay) -> bool) {
        if self.overlay.as_ref().is_some_and(is_this) {
            self.overlay = None;
        }
    }

    pub(super) const fn command(pr_id: PrId, command: Command) -> Effect {
        Effect::Command { pr_id, command }
    }

    pub(super) fn dismiss_error(&mut self, pr_id: PrId) -> Effect {
        self.error = super::dialogs::error::ErrorDialog::default();
        Effect::DismissError { pr_id }
    }

    pub(super) fn submit_editor(&self, pr_id: PrId, ctx: &DetailContext<'_>) -> Option<Effect> {
        let CommentDraft { target, text } = self.editor.draft()?;
        // A blank draft is not a comment: Ctrl+S does nothing until it has text.
        let text = crate::domain::comment::NonBlank::new(text)?;
        let command = match target {
            // A verdict is tied to the commit that was read.
            CommentTarget::Review { verdict } => Command::SubmitReview {
                verdict,
                body: text.into_string(),
                head: match ctx.reviewed_head() {
                    Some(head) => head,
                    None => return Some(head_unknown(pr_id)),
                },
            },
            CommentTarget::Pr
            | CommentTarget::Line(_)
            | CommentTarget::Reply(_)
            | CommentTarget::Edit(_) => Command::SubmitComment { target, text },
        };
        Some(Self::command(pr_id, command))
    }

    /// `w`: show what is new since the reader looked, in the Diff tab, or go
    /// back to the whole diff.
    fn toggle_since(&mut self, pr_id: PrId, ctx: &DetailContext<'_>) -> Option<Effect> {
        if let Some(since) = self.since.take() {
            // A branch that was rewritten or reset is read in the whole diff, and
            // going there from what said so is arriving at it. Leaving any other
            // is only closing it.
            return since
                .sends_to_the_whole_diff(ctx.data)
                .then_some(Effect::Navigate(
                    crate::tui::app::navigation::Screen::Detail {
                        pr_id,
                        tab: super::tabs::DetailTab::Diff,
                    },
                ));
        }
        let range = self.view(ctx).moved_since_read()?;
        self.since = Some(super::since::SinceView::new(range.clone()));
        self.active_tab = super::tabs::DetailTab::Diff;
        Some(Effect::OpenSince { pr_id, range })
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
            PrAction::ToggleSince => return self.toggle_since(pr_id, ctx),
            PrAction::OpenIssues => {
                // With one issue the key opens it, and with none there is
                // nothing to choose.
                if issues_to_open(ctx.data).len() > 1 {
                    self.overlay = Some(Overlay::Issues(IssueDialog::default()));
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
            PrAction::OpenComment => return self.open_comment(pr_id, ctx),
            PrAction::OpenAgentReview if self.view(ctx).agent_reviewing() => {
                // The same key that asked for it stops it, after asking: minutes of
                // work are lost, and what it was going to say is not kept.
                let preview = "What it has found so far is not kept.\n\
                               Leaving the PR does not stop it; quitting slussa does."
                    .to_owned();
                self.ask(ConfirmDialog::new(ConfirmKind::StopAgentReview).with_preview(preview));
                return None;
            }
            PrAction::OpenAgentReview => {
                let command = ctx.store.agent_review.join(" ");
                let instructions = ctx
                    .store
                    .agent_review_instructions
                    .as_ref()
                    .map_or_else(String::new, |path| {
                        format!("Instructions: {}\n", path.display())
                    });
                let preview = format!(
                    "Runs: {command}\n{instructions}Given: the title and description of PR #{pr_id},\n\
                     the issues it closes, the repository's rules\n(AGENTS.md, CLAUDE.md) and the diff.\n\
                     It proposes comments; nothing is posted."
                );
                self.ask(ConfirmDialog::new(ConfirmKind::AgentReview).with_preview(preview));
                return None;
            }
            PrAction::DiscardProposal => {
                let index = self.surface(ctx.tab).diff_viewer()?.focused_proposal()?;
                return Some(Effect::HandleProposal {
                    pr_id,
                    index,
                    how: How::Discarded,
                });
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
            PrAction::RerequestReview => Command::RerequestReview(self.view(ctx).rerequest()?),
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
