use crate::{
    app::{App, action::DetailAction, state::Screen},
    tui::screens::pr_detail::DetailTab,
};

impl App {
    pub(super) fn apply_detail(&mut self, action: DetailAction) {
        match action {
            DetailAction::Back => self.back_to_list(),
            DetailAction::NextTab => self.next_tab(),
            DetailAction::PrevTab => self.prev_tab(),
            DetailAction::SelectTab(t) => self.select_tab(t),
            DetailAction::DescriptionScrollDown => self.description_scroll_down(),
            DetailAction::DescriptionScrollUp => self.description_scroll_up(),
            DetailAction::OverviewScrollDown => self.overview_scroll_down(),
            DetailAction::OverviewScrollUp => self.overview_scroll_up(),
        }
    }

    fn back_to_list(&mut self) {
        self.state.screen = Screen::List;
    }

    fn next_tab(&mut self) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = tab.next();
        }
    }

    fn prev_tab(&mut self) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = tab.prev();
        }
    }

    fn select_tab(&mut self, new_tab: DetailTab) {
        if let Screen::Detail { tab, .. } = &mut self.state.screen {
            *tab = new_tab;
        }
    }

    fn description_scroll_down(&mut self) {
        self.state.ui.description_scroll = self.state.ui.description_scroll.saturating_add(1);
    }

    fn description_scroll_up(&mut self) {
        self.state.ui.description_scroll = self.state.ui.description_scroll.saturating_sub(1);
    }

    fn overview_scroll_down(&mut self) {
        self.state.ui.overview_scroll = self.state.ui.overview_scroll.saturating_add(1);
    }

    fn overview_scroll_up(&mut self) {
        self.state.ui.overview_scroll = self.state.ui.overview_scroll.saturating_sub(1);
    }
}
