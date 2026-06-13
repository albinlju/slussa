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
    action::{Action, SearchAction},
    state::{AppState, Screen, SearchState, SearchTarget},
};

use screens::{pr_detail, pr_list};

pub fn render(frame: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::List => pr_list::render(frame, state, frame.area()),
        Screen::Detail { pr_id, tab } => pr_detail::render(frame, state, pr_id, tab, frame.area()),
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if let Some((search, highlight)) = active_search(state) {
        if search.open {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char(c) => return Some(Action::Search(SearchAction::Type(c))),
                    KeyCode::Backspace => return Some(Action::Search(SearchAction::Backspace)),
                    KeyCode::Esc => return Some(Action::Search(SearchAction::Cancel)),
                    KeyCode::Enter if highlight => {
                        return Some(Action::Search(SearchAction::Confirm));
                    }
                    _ => {}
                }
            }
        } else if key.code == KeyCode::Char('/') {
            return Some(Action::Search(SearchAction::Open));
        }
    }

    match state.screen {
        Screen::List => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
    }
}

fn active_search(state: &AppState) -> Option<(&SearchState, bool)> {
    let ui = &state.ui;
    Some(match state.search_target()? {
        SearchTarget::List => (&ui.list_search, false),
        SearchTarget::Commits => (&ui.commits.search, false),
        SearchTarget::DiffTree => (&ui.active_diff_view().tree_search, false),
        SearchTarget::DiffPane => (&ui.active_diff_view().pane_search, true),
    })
}
