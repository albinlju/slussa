pub mod screens;
pub mod theme;
pub mod widgets;

use ratatui::{Frame, crossterm::event::KeyEvent};

use crate::app::{
    action::Action,
    state::{AppState, Screen},
};

use screens::{pr_detail, pr_list};

pub fn render(frame: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::List => pr_list::render(frame, state, frame.area()),
        Screen::Detail { pr_id, tab } => pr_detail::render(frame, state, pr_id, tab, frame.area()),
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    match state.screen {
        Screen::List => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
    }
}
