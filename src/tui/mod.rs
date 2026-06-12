pub mod format;
pub mod markdown;
pub mod screens;
pub mod theme;
pub mod widgets;

use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

use crate::app::{
    action::{Action, SearchInput},
    state::{AppState, DetailTab, DiffFocus, Screen, SearchState},
};

use screens::{pr_detail, pr_list};

pub fn render(frame: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::List => pr_list::render(frame, state, frame.area()),
        Screen::Detail { pr_id, tab } => pr_detail::render(frame, state, pr_id, tab, frame.area()),
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    // Generic `/` search, shared by every searchable view: `/` opens it, then
    // letters are query text while Esc/Backspace edit it. Navigation (arrows,
    // Enter, paging) falls through to the screen handler unchanged.
    if let Some((search, highlight)) = active_search(state) {
        if search.open {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char(c) => return Some(Action::Search(SearchInput::Type(c))),
                    KeyCode::Backspace => return Some(Action::Search(SearchInput::Backspace)),
                    KeyCode::Esc => return Some(Action::Search(SearchInput::Cancel)),
                    // The pane keeps its highlight on Enter; filter views let
                    // Enter fall through to select/open the highlighted row.
                    KeyCode::Enter if highlight => {
                        return Some(Action::Search(SearchInput::Confirm));
                    }
                    _ => {}
                }
            }
        } else if key.code == KeyCode::Char('/') {
            return Some(Action::Search(SearchInput::Open));
        }
    }

    match state.screen {
        Screen::List => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
    }
}

/// The `SearchState` the active view drives, plus whether it's a *highlight*
/// search (the diff pane) vs a filter search (everything else). Mirror of
/// `App::active_search_mut` in the reducer.
fn active_search(state: &AppState) -> Option<(&SearchState, bool)> {
    match state.screen {
        // The filter picker is a modal that owns the keyboard while open.
        Screen::List => (!state.ui.filter_picker_open).then_some((&state.ui.list_search, false)),
        Screen::Detail { tab, .. } => {
            let drilled = state.ui.commits.drilled.is_some();
            match tab {
                DetailTab::Commits if !drilled => Some((&state.ui.commits.search, false)),
                DetailTab::Diff | DetailTab::Commits => {
                    let view = if drilled {
                        &state.ui.commits.diff
                    } else {
                        &state.ui.diff
                    };
                    match view.focus {
                        DiffFocus::Tree => Some((&view.tree_search, false)),
                        DiffFocus::Pane => Some((&view.pane_search, true)),
                    }
                }
                _ => None,
            }
        }
    }
}
