use ratatui::{Frame, layout::Rect};

use crate::{
    app::state::UiMemory,
    domain::pr::PullRequest,
    tui::{markdown, widgets},
};

pub fn render(frame: &mut Frame, pr: &PullRequest, ui: &mut UiMemory, area: Rect) {
    let lines = markdown::render(description_body(pr), area.width.saturating_sub(1));
    widgets::scrolled_paragraph(
        frame,
        lines,
        &mut ui.description_scroll,
        &mut ui.description_viewport,
        area,
    );
}

fn description_body(pr: &PullRequest) -> &str {
    pr.description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no description)")
}
