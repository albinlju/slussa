use crate::app::{
    App,
    action::DetailAction,
    state::{CommentDraft, DetailTab, DiffFocus, Screen},
};

impl App {
    pub(super) fn apply_detail(&mut self, action: DetailAction) {
        match action {
            DetailAction::Back => self.back_to_list(),
            DetailAction::NextTab => self.next_tab(),
            DetailAction::PrevTab => self.prev_tab(),
            DetailAction::SelectTab(t) => self.select_tab(t),
            DetailAction::DescriptionScroll(delta) => {
                self.state.ui.description_scroll =
                    super::scroll(self.state.ui.description_scroll, delta);
            }
            DetailAction::OverviewMove(delta) => {
                self.state.ui.overview_cursor = super::step_index(
                    self.state.ui.overview_cursor,
                    delta,
                    self.state.ui.overview_item_count,
                );
            }
            DetailAction::ToggleHelp => self.state.ui.help_open = !self.state.ui.help_open,
            DetailAction::OpenConfirm(kind) => {
                self.state.ui.confirm = Some(kind);
                self.state.ui.confirm_cursor = 0;
            }
            DetailAction::CloseConfirm => self.state.ui.confirm = None,
            DetailAction::ConfirmMove(delta) => {
                self.state.ui.confirm_cursor =
                    super::step_index(self.state.ui.confirm_cursor, delta, 2);
            }
            DetailAction::SubmitConfirm => self.submit_confirm(),
            DetailAction::OpenComment => self.open_comment(),
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

    fn open_comment(&mut self) {
        let Some(target) = self.state.comment_target() else {
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
        if let Screen::Detail { pr_id, .. } = self.state.screen {
            self.state.ui.comment_pending = true;
            self.spawn_comment(pr_id, draft.target, draft.text);
        }
    }

    fn submit_confirm(&mut self) {
        if self.state.ui.confirm_cursor == 0 {
            // cursor 0 = Yes. TODO: match on self.state.ui.confirm and spawn the
            // write (currently only ConfirmKind::Approve).
        }
        self.state.ui.confirm = None;
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
