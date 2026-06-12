//! The one place `/` search is edited. `Action::Search` carries no target —
//! `AppState::search_target` routes it to whichever view's `SearchState` is
//! active. Each view only supplies *what* it matches against; the
//! typing/clearing logic lives here and nowhere else.

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
            // Open/Confirm don't change the result set, so no selection reset.
            SearchInput::Open => {
                search.open = true;
                return;
            }
            SearchInput::Confirm => {
                search.open = false; // keep the query (e.g. the pane's highlight)
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
        // The query changed → jump the filter view's selection to the top
        // match. (The pane highlights rather than filters; nothing to reset.)
        match target {
            SearchTarget::List => self.state.ui.list_selected = 0,
            SearchTarget::Commits => self.state.ui.commits.selected = 0,
            SearchTarget::DiffTree => {
                self.state.ui.active_diff_view_mut().cursor = 0;
                // Keep the pane on the first matching file.
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
