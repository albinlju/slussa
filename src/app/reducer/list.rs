use crate::app::{
    App,
    action::ListAction,
    state::{CommitsViewState, DetailTab, DiffViewState, Screen, SearchState, StatusFilter},
};

impl App {
    pub(super) fn apply_list(&mut self, action: ListAction) {
        match action {
            ListAction::MoveSelection(delta) => self.move_selection(delta),
            ListAction::OpenPr(pr_id) => self.open_pr(pr_id),
            ListAction::OpenFilterPicker => self.open_filter_picker(),
            ListAction::CloseFilterPicker => self.close_filter_picker(),
            ListAction::FilterPickerNext => self.filter_picker_next(),
            ListAction::FilterPickerPrev => self.filter_picker_prev(),
            ListAction::ApplyFilter => self.apply_filter(),
        }
    }

    fn move_selection(&mut self, delta: i16) {
        let len = self.state.filtered_prs().len();
        self.state.ui.list_selected = super::step_index(self.state.ui.list_selected, delta, len);
    }

    fn open_pr(&mut self, pr_id: u64) {
        self.state.screen = Screen::Detail {
            pr_id,
            tab: DetailTab::default(),
        };
        self.state.ui.list_search = SearchState::default();
        self.state.ui.diff = DiffViewState::default();
        self.state.ui.commits = CommitsViewState::default();
        self.state.ui.description_scroll = 0;
        self.state.ui.overview_scroll = 0;

        let pr_data = self.state.cache.details.entry(pr_id).or_default();
        let load_commits = pr_data.commits.start_loading();
        let load_diff = pr_data.diff.start_loading();
        let load_builds = pr_data.builds.start_loading();
        let load_activity = pr_data.activity.start_loading();
        if load_commits {
            self.spawn_load_commits(pr_id);
        }
        if load_diff {
            self.spawn_load_diff(pr_id);
        }
        if load_builds {
            self.spawn_load_builds(pr_id);
        }
        if load_activity {
            self.spawn_load_activity(pr_id);
        }
    }

    fn open_filter_picker(&mut self) {
        self.state.ui.filter_picker_cursor = StatusFilter::CYCLE
            .iter()
            .position(|&f| f == self.state.ui.list_filter)
            .unwrap_or(0);
        self.state.ui.filter_picker_open = true;
    }

    fn close_filter_picker(&mut self) {
        self.state.ui.filter_picker_open = false;
    }

    fn filter_picker_next(&mut self) {
        let last = StatusFilter::CYCLE.len().saturating_sub(1);
        self.state.ui.filter_picker_cursor = (self.state.ui.filter_picker_cursor + 1).min(last);
    }

    fn filter_picker_prev(&mut self) {
        self.state.ui.filter_picker_cursor = self.state.ui.filter_picker_cursor.saturating_sub(1);
    }

    fn apply_filter(&mut self) {
        let new_filter = StatusFilter::CYCLE
            .get(self.state.ui.filter_picker_cursor)
            .copied()
            .unwrap_or(StatusFilter::Open);
        if new_filter != self.state.ui.list_filter {
            self.state.ui.list_filter = new_filter;
            self.state.ui.list_selected = 0;
        }
        self.state.ui.filter_picker_open = false;
    }
}
