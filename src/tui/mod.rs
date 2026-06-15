pub mod format;
pub mod icons;
pub mod layout;
pub mod markdown;
pub mod screens;
pub mod table;
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
    let key = normalize_key(key);
    if let Some((search, highlight)) = active_search(state)
        && let Some(action) = search_action(search, highlight, key)
    {
        return Some(action);
    }

    match state.screen {
        Screen::List => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
    }
}

fn search_action(search: &SearchState, highlight: bool, key: KeyEvent) -> Option<Action> {
    if !search.open {
        return (key.code == KeyCode::Char('/')).then_some(Action::Search(SearchAction::Open));
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match key.code {
        KeyCode::Char(c) => Some(Action::Search(SearchAction::Type(c))),
        KeyCode::Backspace => Some(Action::Search(SearchAction::Backspace)),
        KeyCode::Esc => Some(Action::Search(SearchAction::Cancel)),
        KeyCode::Enter if highlight => Some(Action::Search(SearchAction::Confirm)),
        _ => None,
    }
}

fn normalize_key(mut key: KeyEvent) -> KeyEvent {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => key.code = KeyCode::PageDown,
            KeyCode::Char('u') => key.code = KeyCode::PageUp,
            _ => return key,
        }
        key.modifiers.remove(KeyModifiers::CONTROL);
    }
    key
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
