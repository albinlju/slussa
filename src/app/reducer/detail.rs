use crate::app::{
    App,
    action::DetailAction,
    state::{DetailTab, DiffFocus, Screen},
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
            DetailAction::OverviewScroll(delta) => {
                self.state.ui.overview_scroll = super::scroll(self.state.ui.overview_scroll, delta);
            }
            DetailAction::ToggleHelp => self.state.ui.help_open = !self.state.ui.help_open,
            DetailAction::OpenApprove => {
                self.state.ui.approve_box_open = true;
                self.state.ui.approve_box_cursor = 0;
            }
            DetailAction::CloseApprove => self.state.ui.approve_box_open = false,
            DetailAction::ApproveMove(delta) => {
                self.state.ui.approve_box_cursor =
                    super::step_index(self.state.ui.approve_box_cursor, delta, 2);
            }
            DetailAction::SubmitApprove => self.submit_approve(),
        }
    }

    fn submit_approve(&mut self) {
        // cursor 0 = Yes, 1 = No
        if self.state.ui.approve_box_cursor == 0 {
            // TODO: call the provider approve write here.
        }
        self.state.ui.approve_box_open = false;
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
