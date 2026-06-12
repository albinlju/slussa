//! Markdown → ratatui lines via charmed-glamour. A glamour panic or
//! ANSI-bridge failure falls back to the raw body instead of taking down
//! the TUI.

use ratatui::text::Line;

/// Glamour's Dark theme adds a fixed document margin this wide to every
/// rendered line.
const GLAMOUR_MARGIN: usize = 2;

/// Rendered markdown wrapped to `width`, glamour's own margins kept — used
/// where the margin doubles as the view's indent (the Description tab).
pub(super) fn render(body: &str, width: u16) -> Vec<Line<'static>> {
    trim_blank_lines(glamour_lines(body, width))
}

/// Rendered markdown with glamour's left margin stripped, so the text sits
/// flush against whatever frames it (comment boxes). Text still wraps to
/// `width`.
pub(super) fn render_flush(body: &str, width: u16) -> Vec<Line<'static>> {
    let lines = glamour_lines(body, width + GLAMOUR_MARGIN as u16);
    trim_blank_lines(strip_margin(lines, GLAMOUR_MARGIN))
}

fn glamour_lines(body: &str, width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::default()];
    }
    let rendered = std::panic::catch_unwind(|| {
        let ansi = glamour::Renderer::new()
            .with_style(glamour::Style::Dark)
            .with_word_wrap(width as usize)
            .render(body);
        ansi_to_tui::IntoText::into_text(&ansi).map(|text| text.lines)
    });
    let lines = match rendered {
        Ok(Ok(lines)) => lines,
        _ => body.lines().map(|l| Line::raw(l.to_string())).collect(),
    };
    if lines.is_empty() {
        vec![Line::default()]
    } else {
        lines
    }
}

/// Strip `n` leading chars from each line's first span.
fn strip_margin(lines: Vec<Line<'static>>, n: usize) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|mut line| {
            if let Some(first) = line.spans.first_mut() {
                let trimmed: String = first.content.chars().skip(n).collect();
                first.content = trimmed.into();
            }
            line
        })
        .collect()
}

fn trim_blank_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    while lines.first().is_some_and(is_blank_line) {
        lines.remove(0);
    }
    while lines.last().is_some_and(is_blank_line) {
        lines.pop();
    }
    lines
}

fn is_blank_line(line: &Line<'static>) -> bool {
    line.spans.is_empty() || line.spans.iter().all(|s| s.content.trim().is_empty())
}
