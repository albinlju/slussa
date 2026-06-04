use ratatui::{Frame, crossterm::event::KeyCode};

use crate::{app::state::AppState, tui::Outcome};

pub fn render(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {}

pub fn handle_key(state: &mut AppState, key: KeyCode) -> Outcome {
    match key {
        KeyCode::Char('q') => Outcome::Quit,
        _ => Outcome::Continue,
    }
}
