//! Handlers for `ListAction` — actions on the PR list screen (cursor
//! navigation, opening a PR, filter-picker popup).

use crate::{
    app::{
        App,
        action::ListAction,
        state::{DiffViewState, Screen, StatusFilter},
    },
    tui::screens::pr_detail::DetailTab,
};

impl App {
    pub(super) fn apply_list(&mut self, action: ListAction) {
        match action {
            ListAction::NextPr => self.next_pr(),
            ListAction::PrevPr => self.prev_pr(),
            ListAction::OpenPr(pr_id) => self.open_pr(pr_id),
            ListAction::OpenFilterPicker => self.open_filter_picker(),
            ListAction::CloseFilterPicker => self.close_filter_picker(),
            ListAction::FilterPickerNext => self.filter_picker_next(),
            ListAction::FilterPickerPrev => self.filter_picker_prev(),
            ListAction::ApplyFilter => self.apply_filter(),
        }
    }

    fn next_pr(&mut self) {
        let len = self.state.filtered_prs().len();
        let last = len.saturating_sub(1);
        self.state.ui.list_selected = (self.state.ui.list_selected + 1).min(last);
    }

    fn prev_pr(&mut self) {
        self.state.ui.list_selected = self.state.ui.list_selected.saturating_sub(1);
    }

    fn open_pr(&mut self, pr_id: u64) {
        self.state.screen = Screen::Detail {
            pr_id,
            tab: DetailTab::default(),
        };
        self.state.ui.diff = DiffViewState::default();
        self.state.ui.description_scroll = 0;
        self.state.ui.overview_scroll = 0;

        // Kick off each background fetch only on the first open — `start_loading`
        // returns `true` exactly when the LoadState was `NotRequested`.
        let pr_data = self.state.cache.details.entry(pr_id).or_default();
        let load_commits = pr_data.commits.start_loading();
        let load_diff = pr_data.diff.start_loading();
        let load_comments = pr_data.comments.start_loading();
        let load_threads = pr_data.review_threads.start_loading();
        if load_commits {
            self.spawn_load_commits(pr_id);
        }
        if load_diff {
            self.spawn_load_diff(pr_id);
        }
        if load_comments {
            self.spawn_load_comments(pr_id);
        }
        if load_threads {
            self.spawn_load_review_threads(pr_id);
        }
    }

    fn open_filter_picker(&mut self) {
        // Start the picker cursor on the currently active filter so pressing
        // Enter without moving keeps the same filter.
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
        self.state.ui.filter_picker_cursor =
            (self.state.ui.filter_picker_cursor + 1).min(last);
    }

    fn filter_picker_prev(&mut self) {
        self.state.ui.filter_picker_cursor =
            self.state.ui.filter_picker_cursor.saturating_sub(1);
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
