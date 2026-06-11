//! The one place `/` search is edited. `Action::Search` carries no target —
//! the reducer routes it to whichever view's `SearchState` is active (PR list,
//! commit list, file tree), mirroring `tui::active_search` on the read side.
//! Each view only supplies *what* it matches against; the typing/clearing logic
//! lives here and nowhere else.

use crate::{
    app::{
        App,
        action::SearchInput,
        state::{DiffFocus, Screen, SearchState},
    },
    tui::screens::pr_detail::{DetailTab, file_tree::TreeRow},
};

impl App {
    pub(super) fn apply_search(&mut self, input: SearchInput) {
        {
            let Some(search) = self.active_search_mut() else {
                return;
            };
            match input {
                // Open/Confirm don't change the result set, so skip the reset.
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
        }
        // The query changed → jump the active filter view's selection to the
        // top match. (No-op for the pane, which highlights rather than filters.)
        self.reset_active_selection();
    }

    fn active_search_mut(&mut self) -> Option<&mut SearchState> {
        match self.state.screen {
            Screen::List => Some(&mut self.state.ui.list_search),
            Screen::Detail { tab, .. } => {
                let drilled = self.state.ui.commits.drilled.is_some();
                match tab {
                    DetailTab::Commits if !drilled => Some(&mut self.state.ui.commits.search),
                    DetailTab::Diff | DetailTab::Commits => {
                        let view = self.diff_view_mut();
                        Some(match view.focus {
                            // Tree filters; pane highlights — both edited here.
                            DiffFocus::Tree => &mut view.tree_search,
                            DiffFocus::Pane => &mut view.pane_search,
                        })
                    }
                    _ => None,
                }
            }
        }
    }

    fn reset_active_selection(&mut self) {
        match self.state.screen {
            Screen::List => self.state.ui.list_selected = 0,
            Screen::Detail { tab, .. } => {
                let drilled = self.state.ui.commits.drilled.is_some();
                match tab {
                    DetailTab::Commits if !drilled => self.state.ui.commits.selected = 0,
                    // Only the tree filters; the pane highlights, so there's
                    // nothing to re-select there.
                    DetailTab::Diff | DetailTab::Commits
                        if self.diff_view().focus == DiffFocus::Tree =>
                    {
                        self.diff_view_mut().cursor = 0;
                        // Keep the pane on the first matching file.
                        if let Some(TreeRow::File { file_index, .. }) =
                            self.current_visible_rows().first()
                        {
                            let idx = *file_index;
                            self.focus_file(idx);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
