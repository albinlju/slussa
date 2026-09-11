use crate::{
    app::{
        App,
        action::DetailAction,
        navigation::Screen,
        reviews::{CommentTarget, PendingComment},
    },
    domain::review::ReviewVerdict,
    tui::{
        components::comment_editor::CommentDraft,
        screens::pr_detail::dialogs::{
            confirm::{ConfirmDialog, ConfirmKind},
            review::ReviewDialog,
        },
    },
};

impl App {
    pub(super) fn detail_actions(&mut self, action: DetailAction) {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        match action {
            DetailAction::SubmitConfirm => self.submit_confirm(),
            DetailAction::OpenReviewPicker => {
                self.state.ui.detail.review_picker = Some(ReviewDialog::new(
                    &self.state.detail_view().review_context(),
                ));
            }
            DetailAction::ReviewSelect => self.select_review(),
            DetailAction::StartReview => {
                self.state.store.reviews.entry(pr_id).or_default();
            }
            DetailAction::FinishReview => {
                if self.state.store.reviews.contains_key(&pr_id) {
                    self.state.ui.detail.review_picker = Some(ReviewDialog::default());
                }
            }
            DetailAction::AbandonReview => {
                self.state.store.reviews.remove(&pr_id);
            }
            DetailAction::RemovePendingComment => self.remove_pending_comment(),
            DetailAction::MergeSelect => self.select_merge(),
            DetailAction::OpenComment => self.open_comment(),
            DetailAction::OpenReply => self.open_reply(),
            DetailAction::EditComment => self.edit_selected_comment(),
            DetailAction::DeleteComment => self.delete_selected_comment(),
            DetailAction::ResolveThread => self.toggle_resolve_thread(),
            DetailAction::CommentSubmit => self.submit_comment(),
            _ => unreachable!("local detail action handled by component"),
        }
    }

    fn edit_selected_comment(&mut self) {
        let Some(sel) = self.state.detail_view().editable_selected() else {
            return;
        };
        let Some(id) = sel.id else {
            return;
        };
        let Some(content) = self
            .state
            .detail_view()
            .find_comment(id)
            .map(|c| c.content.clone())
        else {
            return;
        };
        // Prefill the draft with the current text so the user edits in place.
        self.state.ui.detail.editor.draft = Some(CommentDraft {
            target: CommentTarget::Edit {
                id,
                review: sel.review,
            },
            text: content,
        });
    }

    fn toggle_resolve_thread(&mut self) {
        let Some(thread) = self.state.detail_view().focused_thread() else {
            return;
        };
        // Bitbucket needs a comment id, GitHub a node id — bail if neither is set.
        if thread.node_id.is_none() && thread.comment_id.is_none() {
            return;
        }
        let node_id = thread.node_id.clone();
        let comment_id = thread.comment_id;
        let resolved = !thread.resolved;
        if let Screen::Detail { pr_id, .. } = self.state.screen {
            self.state.ui.detail.comment_pending = true;
            self.spawn_resolve_thread(pr_id, node_id, comment_id, resolved);
        }
    }

    fn delete_selected_comment(&mut self) {
        let Some(sel) = self.state.detail_view().editable_selected() else {
            return;
        };
        let Some(id) = sel.id else {
            return;
        };
        self.state.ui.detail.confirm = Some(ConfirmDialog::new(ConfirmKind::DeleteComment {
            id,
            review: sel.review,
        }));
    }

    fn open_comment(&mut self) {
        self.open_draft(self.state.detail_view().comment_target());
    }

    fn open_reply(&mut self) {
        self.open_draft(self.state.detail_view().reply_target());
    }

    fn open_draft(&mut self, target: Option<CommentTarget>) {
        let Some(target) = target else {
            return;
        };
        self.state.ui.detail.editor.draft = Some(CommentDraft {
            target,
            text: String::new(),
        });
    }

    fn submit_comment(&mut self) {
        let Some(draft) = self.state.ui.detail.editor.draft.take() else {
            return;
        };
        if draft.text.trim().is_empty() {
            return;
        }
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        // A line comment written during a review queues locally and flushes with
        // the verdict; it isn't posted now. Everything else posts immediately.
        if let CommentTarget::Line(anchor) = &draft.target
            && let Some(review) = self.state.store.reviews.get_mut(&pr_id)
        {
            review.comments.push(PendingComment {
                anchor: anchor.clone(),
                text: draft.text,
            });
            return;
        }
        match draft.target {
            // A review summary submits the verdict (plus any queued comments).
            CommentTarget::Review { verdict } => {
                self.submit_review_verdict(pr_id, verdict, draft.text);
            }
            target => {
                self.state.ui.detail.comment_pending = true;
                self.spawn_comment(pr_id, target, draft.text);
            }
        }
    }

    fn select_review(&mut self) {
        let Some(verdict) = self
            .state
            .ui
            .detail
            .review_picker
            .as_ref()
            .and_then(|dialog| dialog.selected(&self.state.detail_view().review_context()))
        else {
            return;
        };
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        self.state.ui.detail.review_picker = None;
        if verdict.needs_body() {
            self.open_draft(Some(CommentTarget::Review { verdict }));
        } else {
            self.submit_review_verdict(pr_id, verdict, String::new());
        }
    }

    /// Remove the queued review comment the diff cursor is on (`d` in the pane).
    fn remove_pending_comment(&mut self) {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        let Some(idx) = self.state.ui.detail.active_diff_view().pane_pending else {
            return;
        };
        if let Some(review) = self.state.store.reviews.get_mut(&pr_id)
            && idx < review.comments.len()
        {
            review.comments.remove(idx);
        }
    }

    /// Merge the PR with the picked strategy and refetch so the new status shows.
    fn select_merge(&mut self) {
        let Some(strategy) = self
            .state
            .ui
            .detail
            .merge_picker
            .take()
            .and_then(|dialog| dialog.selected(&self.state.store.merge_strategies))
        else {
            return;
        };
        if let Screen::Detail { pr_id, .. } = self.state.screen {
            self.state.ui.detail.comment_pending = true;
            self.spawn_merge(pr_id, strategy);
        }
    }

    /// Submit the chosen verdict. If a review is in progress, its queued line
    /// comments flush in the same call; otherwise it's a bare verdict (`a`).
    fn submit_review_verdict(&mut self, pr_id: u64, verdict: ReviewVerdict, body: String) {
        self.state.ui.detail.comment_pending = true;
        let user = self.state.store.current_user.clone();
        match self.state.store.reviews.remove(&pr_id) {
            Some(review) => {
                self.spawn_submit_full_review(pr_id, verdict, body, user, review.comments);
            }
            None => self.spawn_submit_review(pr_id, verdict, body, user),
        }
    }

    fn submit_confirm(&mut self) {
        let confirm = self
            .state
            .ui
            .detail
            .confirm
            .take()
            .and_then(|dialog| dialog.accepted());
        match confirm {
            Some(ConfirmKind::DeleteComment { id, review }) => {
                if let Screen::Detail { pr_id, .. } = self.state.screen {
                    self.state.ui.detail.comment_pending = true;
                    self.spawn_delete_comment(pr_id, id, review);
                }
            }
            Some(ConfirmKind::Decline) => {
                if let Screen::Detail { pr_id, .. } = self.state.screen {
                    self.state.ui.detail.comment_pending = true;
                    self.spawn_decline(pr_id);
                }
            }
            None => {}
        }
    }
}
