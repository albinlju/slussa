use crate::{
    app::{App, action::DetailAction, state::{DiffFocus, Screen}},
    tui::screens::pr_detail::DetailTab,
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
        }
    }

    fn back_to_list(&mut self) {
        self.state.screen = Screen::List;
    }

    fn next_tab(&mut self) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = tab.next();
        }
        self.state.ui.diff.focus = DiffFocus::Tree;
    }

    fn prev_tab(&mut self) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = tab.prev();
        }
        self.state.ui.diff.focus = DiffFocus::Tree;
    }

    fn select_tab(&mut self, new_tab: DetailTab) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = new_tab;
        }
        self.state.ui.diff.focus = DiffFocus::Tree;
    }
}
