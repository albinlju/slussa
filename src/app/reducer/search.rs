use crate::app::{
    App,
    action::SearchInput,
    file_tree::TreeRow,
    state::{SearchState, SearchTarget},
};

impl App {
    pub(super) fn apply_search(&mut self, input: SearchInput) {
        let Some(target) = self.state.search_target() else {
            return;
        };
        let search = self.search_mut(target);
        match input {
            SearchInput::Open => {
                search.open = true;
                return;
            }
            SearchInput::Confirm => {
                search.open = false;
                return;
            }
            SearchInput::Type(c) => search.query.push(c),
            SearchInput::Backspace => {
                search.query.pop();
            }
            SearchInput::Cancel => {
                search.open = false;
                search.query.clear();
            }
        }
        match target {
            SearchTarget::List => self.state.ui.list_selected = 0,
            SearchTarget::Commits => self.state.ui.commits.selected = 0,
            SearchTarget::DiffTree => {
                self.state.ui.active_diff_view_mut().cursor = 0;
                if let Some(TreeRow::File { file_index, .. }) =
                    self.current_visible_rows().first()
                {
                    let idx = *file_index;
                    self.focus_file(idx);
                }
            }
            SearchTarget::DiffPane => {}
        }
    }

    fn search_mut(&mut self, target: SearchTarget) -> &mut SearchState {
        match target {
            SearchTarget::List => &mut self.state.ui.list_search,
            SearchTarget::Commits => &mut self.state.ui.commits.search,
            SearchTarget::DiffTree => &mut self.state.ui.active_diff_view_mut().tree_search,
            SearchTarget::DiffPane => &mut self.state.ui.active_diff_view_mut().pane_search,
        }
    }
}
