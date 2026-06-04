pub mod pr_detail;
pub mod pr_list;

use ratatui::Frame;

use crate::app::state::{AppState, Mode};

pub enum Outcome {
    Continue,
    Quit,
}

pub fn render(frame: &mut Frame, state: &AppState) {
    match state.mode {
        Mode::PrList => pr_list::render(frame, state, frame.area()),
        Mode::PrDetail => pr_detail::render(frame, state, frame.area()),
    }
}
