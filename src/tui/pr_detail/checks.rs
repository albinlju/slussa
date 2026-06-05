use ratatui::{Frame, layout::Rect};

use super::render_placeholder;

pub fn render(frame: &mut Frame, area: Rect) {
    render_placeholder(frame, "Checks — TODO", area);
}
