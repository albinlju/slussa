use crate::app::{
    App,
    action::DetailAction,
    state::{CommentDraft, CommentTarget, ConfirmKind, DetailTab, DiffFocus, Screen},
};

impl App {
    pub(super) fn detail_actions(&mut self, action: DetailAction) {
        match action {
            DetailAction::Back => self.back_to_list(),
            DetailAction::NextTab => self.next_tab(),
            DetailAction::PrevTab => self.prev_tab(),
            DetailAction::SelectTab(t) => self.select_tab(t),
            DetailAction::DescriptionScroll(delta) => {
                self.state.ui.description_scroll =
                    super::scroll(self.state.ui.description_scroll, delta);
            }
            DetailAction::OverviewMove(delta) => self.overview_move(delta),
            DetailAction::ToggleHelp => self.state.ui.help_open = !self.state.ui.help_open,
            DetailAction::CloseConfirm => self.state.ui.confirm = None,
            DetailAction::ConfirmMove(delta) => {
                self.state.ui.confirm_cursor =
                    super::step_index(self.state.ui.confirm_cursor, delta, 2);
            }
            DetailAction::SubmitConfirm => self.submit_confirm(),
            DetailAction::OpenReviewPicker => {
                self.state.ui.review_picker = Some(0);
            }
            DetailAction::ReviewMove(delta) => {
                if let Some(cursor) = self.state.ui.review_picker {
                    let len = self.state.review_verdicts().len();
                    self.state.ui.review_picker = Some(super::step_index(cursor, delta, len));
                }
            }
            DetailAction::ReviewSelect => self.select_review(),
            DetailAction::CloseReviewPicker => self.state.ui.review_picker = None,
            DetailAction::OpenComment => self.open_comment(),
            DetailAction::OpenReply => self.open_reply(),
            DetailAction::OverviewSubMove(delta) => self.overview_sub_move(delta),
            DetailAction::EditComment => self.edit_selected_comment(),
            DetailAction::DeleteComment => self.delete_selected_comment(),
            DetailAction::ResolveThread => self.toggle_resolve_thread(),
            DetailAction::DismissError => self.state.ui.error = None,
            DetailAction::CommentType(c) => {
                if let Some(draft) = &mut self.state.ui.comment_draft {
                    draft.text.push(c);
                }
            }
            DetailAction::CommentBackspace => {
                if let Some(draft) = &mut self.state.ui.comment_draft {
                    draft.text.pop();
                }
            }
            DetailAction::CommentSubmit => self.submit_comment(),
            DetailAction::CommentCancel => self.state.ui.comment_draft = None,
        }
    }

    fn overview_move(&mut self, delta: i16) {
        let ui = &mut self.state.ui;
        // j/k jumps between comments only when there are at least two to jump
        // between; with 0 or 1 there's nothing to navigate, so scroll the
        // timeline directly (and events outside the comment stay reachable).
        if ui.overview_item_count <= 1 {
            ui.overview_scroll = super::scroll(ui.overview_scroll, delta);
        } else {
            let next = super::step_index(ui.overview_cursor, delta, ui.overview_item_count);
            if next != ui.overview_cursor {
                ui.overview_cursor = next;
                ui.overview_sub = 0; // new block → back to its first comment
            }
        }
    }

    fn overview_sub_move(&mut self, delta: i16) {
        let ui = &mut self.state.ui;
        ui.overview_sub = super::step_index(ui.overview_sub, delta, ui.overview_block_len);
    }

    fn edit_selected_comment(&mut self) {
        let Some(sel) = self.state.editable_selected() else {
            return;
        };
        let Some(id) = sel.id else {
            return;
        };
        let Some(content) = self.state.find_comment(id).map(|c| c.content.clone()) else {
            return;
        };
        // Prefill the draft with the current text so the user edits in place.
        self.state.ui.comment_draft = Some(CommentDraft {
            target: CommentTarget::Edit {
                id,
                review: sel.review,
            },
            text: content,
        });
    }

    fn toggle_resolve_thread(&mut self) {
        let Some(thread) = self.state.focused_thread() else {
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
            self.state.ui.comment_pending = true;
            self.spawn_resolve_thread(pr_id, node_id, comment_id, resolved);
        }
    }

    fn delete_selected_comment(&mut self) {
        let Some(sel) = self.state.editable_selected() else {
            return;
        };
        let Some(id) = sel.id else {
            return;
        };
        self.state.ui.confirm = Some(ConfirmKind::DeleteComment {
            id,
            review: sel.review,
        });
        self.state.ui.confirm_cursor = 0;
    }

    fn open_comment(&mut self) {
        self.open_draft(self.state.comment_target());
    }

    fn open_reply(&mut self) {
        self.open_draft(self.state.reply_target());
    }

    fn open_draft(&mut self, target: Option<CommentTarget>) {
        let Some(target) = target else {
            return;
        };
        self.state.ui.comment_draft = Some(CommentDraft {
            target,
            text: String::new(),
        });
    }

    fn submit_comment(&mut self) {
        let Some(draft) = self.state.ui.comment_draft.take() else {
            return;
        };
        if draft.text.trim().is_empty() {
            return;
        }
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        self.state.ui.comment_pending = true;
        match draft.target {
            // A review summary submits the verdict; everything else is a comment.
            CommentTarget::Review { verdict } => {
                let user = self.state.current_user.clone();
                self.spawn_submit_review(pr_id, verdict, draft.text, user);
            }
            target => self.spawn_comment(pr_id, target, draft.text),
        }
    }

    /// `a` review menu: bodyless verdicts (approve / unapprove) submit straight
    /// away; request-changes / comment open the draft for their summary.
    fn select_review(&mut self) {
        let Some(cursor) = self.state.ui.review_picker.take() else {
            return;
        };
        let Some(verdict) = self.state.review_verdicts().get(cursor).copied() else {
            return;
        };
        if verdict.needs_body() {
            self.open_draft(Some(CommentTarget::Review { verdict }));
        } else if let Screen::Detail { pr_id, .. } = self.state.screen {
            self.state.ui.comment_pending = true;
            let user = self.state.current_user.clone();
            self.spawn_submit_review(pr_id, verdict, String::new(), user);
        }
    }

    fn submit_confirm(&mut self) {
        let confirm = self.state.ui.confirm.take();
        if self.state.ui.confirm_cursor != 0 {
            return; // cursor 0 = Yes
        }
        match confirm {
            Some(ConfirmKind::DeleteComment { id, review }) => {
                if let Screen::Detail { pr_id, .. } = self.state.screen {
                    self.state.ui.comment_pending = true;
                    self.spawn_delete_comment(pr_id, id, review);
                }
            }
            None => {}
        }
    }

    fn back_to_list(&mut self) {
        self.state.screen = Screen::List;
        self.state.ui.help_open = false;
    }

    fn next_tab(&mut self) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = tab.next();
        }
        self.reset_tab_state();
    }

    fn prev_tab(&mut self) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = tab.prev();
        }
        self.reset_tab_state();
    }

    fn select_tab(&mut self, new_tab: DetailTab) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = new_tab;
        }
        self.reset_tab_state();
    }

    fn reset_tab_state(&mut self) {
        self.state.ui.diff.focus = DiffFocus::Tree;
        self.state.ui.commits.open_commit = None;
    }
}
