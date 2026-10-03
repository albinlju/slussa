//! The frames around comments: the header line, the left rail and the box.

use crate::tui::ui::widgets;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub(super) fn header_line(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let width = width as usize;
    if width == 0 {
        return Line::default();
    }
    if right.is_empty() {
        return Line::from(widgets::truncate_to_width(left, width));
    }
    // Reserve room for status/time without letting metadata hide the author
    // entirely in very narrow panes. Measure terminal columns, not bytes.
    let left_min = 12.min(width / 2);
    let right = widgets::truncate_to_width(right, width.saturating_sub(left_min).max(1));
    let right_width: usize = right.iter().map(Span::width).sum();
    let left_width = width.saturating_sub(right_width + 1);
    let left = if left_width == 0 {
        Vec::new()
    } else {
        widgets::truncate_to_width(left, left_width)
    };
    if left.is_empty() {
        return Line::from(right);
    }
    Line::from(widgets::justify_between(left, right, width))
}

/// `left` colours the vertical edge (accent marks focus); `rule` colours the
/// top/bottom — kept muted so focus only lights up the left border.
pub(super) fn bracket(
    body: Vec<Line<'static>>,
    width: u16,
    left: Color,
    rule: Color,
) -> Vec<Line<'static>> {
    let left_style = Style::default().fg(left);
    let rule_line = |corner: &str| {
        Line::from(Span::styled(
            format!("{corner}{}", "─".repeat((width as usize).saturating_sub(1))),
            Style::default().fg(rule),
        ))
    };
    let mut out = vec![rule_line("┌")];
    for line in body {
        let mut spans = vec![Span::styled("| ", left_style)];
        spans.extend(line.spans);
        out.push(Line::from(spans).style(line.style));
    }
    out.push(rule_line("└"));
    out
}

pub(super) fn prefix_gutter(
    line: Line<'static>,
    gutter: &'static str,
    style: Style,
) -> Line<'static> {
    let mut spans = vec![Span::styled(gutter, style)];
    spans.extend(line.spans);
    Line::from(spans).style(line.style)
}

pub(super) fn framed(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    body: Vec<Line<'static>>,
    width: u16,
    left_color: Color,
    rule: Color,
) -> Vec<Line<'static>> {
    let style = Style::default().fg(rule);
    let left_style = Style::default().fg(left_color);
    let w = width as usize;
    let left_w: usize = left.iter().map(Span::width).sum();
    let right_w: usize = right.iter().map(Span::width).sum();

    let mut top = vec![Span::styled("┌─ ", style)];
    top.extend(left);
    if right_w == 0 {
        let fill = w.saturating_sub(left_w + 4).max(1);
        top.push(Span::styled(format!(" {}", "─".repeat(fill)), style));
    } else {
        let fill = w.saturating_sub(left_w + right_w + 7).max(1);
        top.push(Span::styled(format!(" {} ", "─".repeat(fill)), style));
        top.extend(right);
        top.push(Span::styled(" ─", style));
    }

    let mut out = vec![Line::from(top)];
    for line in body {
        let mut spans = vec![Span::styled("| ", left_style)];
        spans.extend(line.spans);
        out.push(Line::from(spans).style(line.style));
    }
    out.push(Line::from(Span::styled(
        format!("└{}", "─".repeat(w.saturating_sub(1))),
        style,
    )));
    out
}

pub(super) fn status_rule(label: Vec<Span<'static>>, width: u16, color: Color) -> Line<'static> {
    let style = Style::default().fg(color);
    let label_w: usize = label.iter().map(Span::width).sum();
    let fill = (width as usize).saturating_sub(label_w + 3).max(1);
    let mut spans = vec![Span::styled(format!("{} ", "─".repeat(fill)), style)];
    spans.extend(label);
    spans.push(Span::styled(" ─", style));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::ui::theme;

    #[test]
    fn long_headers_keep_status_and_time_visible_without_overflow() {
        let left = vec![Span::styled(
            "@reviewer commented on src/非常に長いパス/component.rs:123",
            Style::default().fg(theme::current().link),
        )];
        let right = vec![
            Span::styled("unresolved", Style::default().fg(theme::current().warning)),
            Span::raw(" · 5 min"),
        ];
        for width in [0, 1, 8, 20, 40, 80, 120] {
            let line = header_line(left.clone(), right.clone(), width);
            assert!(line.width() <= width as usize);
            if width >= 40 {
                assert!(line.to_string().ends_with("unresolved · 5 min"));
                assert_eq!(
                    line.spans[line.spans.len() - 2].style.fg,
                    Some(theme::current().warning)
                );
            }
            if width == 40 {
                assert!(line.to_string().starts_with("@reviewer"));
                assert!(line.to_string().contains('…'));
            }
            assert!(header_line(left.clone(), vec![], width).width() <= width as usize);
        }
    }
}
