use ratatui::{Frame, layout::Rect, widgets::Paragraph};

use crate::{
    app::state::UiMemory,
    domain::pr::PullRequest,
    tui::{screens::pr_detail::description_body, widgets},
};

pub fn render(frame: &mut Frame, pr: &PullRequest, ui: &mut UiMemory, area: Rect) {
    let content_width = area.width.saturating_sub(1);
    let lines = widgets::trim_blank_lines(widgets::markdown(description_body(pr), content_width));

    let total = lines.len();
    let visible = area.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;
    let scroll = ui.description_scroll.min(max_scroll);
    ui.description_scroll = scroll;
    ui.description_viewport = area.height;

    let content_area = Rect {
        width: content_width,
        ..area
    };
    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, content_area);

    if max_scroll > 0 {
        let bar = widgets::scrollbar(scroll, max_scroll, area.height);
        frame.render_widget(Paragraph::new(bar), widgets::scrollbar_area(area));
    }
}
