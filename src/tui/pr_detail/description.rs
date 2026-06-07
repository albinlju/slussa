use ratatui::{Frame, layout::Rect, widgets::Paragraph};

use crate::{
    app::state::UiMemory,
    domain::pr::PullRequest,
    tui::pr_detail::{description_body, render_markdown, render_thumb_scrollbar, trim_blank_lines},
};

pub fn render(frame: &mut Frame, pr: &PullRequest, ui: &mut UiMemory, area: Rect) {
    // Reserve the rightmost column for the scrollbar so wrapped markdown
    // doesn't get clipped or overlap the thumb.
    let content_width = area.width.saturating_sub(1);
    let lines = trim_blank_lines(render_markdown(description_body(pr), content_width));

    let total = lines.len();
    let visible = area.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;
    let scroll = ui.description_scroll.min(max_scroll);
    ui.description_scroll = scroll;

    let content_area = Rect { width: content_width, ..area };
    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, content_area);

    if max_scroll > 0 {
        render_thumb_scrollbar(frame, scroll, max_scroll, area);
    }
}
