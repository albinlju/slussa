//! Width, truncation, justification and wrapping of text.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(in crate::tui::ui) fn highlight_query(
    line: Line<'static>,
    query: &str,
    match_style: Style,
) -> Line<'static> {
    if query.is_empty() {
        return line;
    }
    let needle = query.to_lowercase();
    let mut out: Vec<Span<'static>> = Vec::new();
    for span in line.spans {
        let content = span.content.into_owned();
        let lower = content.to_lowercase();
        if content.len() != lower.len() {
            out.push(Span::styled(content, span.style));
            continue;
        }
        let mut start = 0;
        while let Some(rel) = lower[start..].find(&needle) {
            let m = start + rel;
            if m > start {
                out.push(Span::styled(content[start..m].to_string(), span.style));
            }
            let end = m + needle.len();
            out.push(Span::styled(
                content[m..end].to_string(),
                span.style.patch(match_style),
            ));
            start = end;
        }
        if start < content.len() {
            out.push(Span::styled(content[start..].to_string(), span.style));
        }
    }
    Line::from(out)
}

/// A bounded row: reserve secondary text, then ellipsize the primary text.
pub(in crate::tui::ui) fn fitted_row(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: usize,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }
    if right.is_empty() {
        return Line::from(truncate_to_width(left, width));
    }
    let right = truncate_to_width(right, width.saturating_sub(12.min(width / 2)).max(1));
    let right_w: usize = right.iter().map(Span::width).sum();
    let budget = width.saturating_sub(right_w + 1);
    if budget == 0 {
        return Line::from(right);
    }
    let left = truncate_to_width(left, budget);
    Line::from(justify_between(left, right, width))
}

pub(in crate::tui::ui) fn justify_between(
    mut left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: usize,
) -> Vec<Span<'static>> {
    let left_w: usize = left.iter().map(Span::width).sum();
    let right_w: usize = right.iter().map(Span::width).sum();
    let pad = width.saturating_sub(left_w + right_w).max(1);
    left.push(Span::raw(" ".repeat(pad)));
    left.extend(right);
    left
}

pub(in crate::tui::ui) fn truncate_to_width(
    spans: Vec<Span<'static>>,
    max: usize,
) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(Span::width).sum();
    if total <= max {
        return spans;
    }
    let budget = max.saturating_sub(1);
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for span in spans {
        let w = span.width();
        if used + w <= budget {
            used += w;
            out.push(span);
        } else {
            let kept = take_to_width(&span.content, budget - used);
            if !kept.is_empty() {
                out.push(Span::styled(kept, span.style));
            }
            break;
        }
    }
    out.push(Span::raw("…"));
    out
}

fn take_to_width(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = Span::raw(c.to_string()).width();
        if used + w > max {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}

/// Wrap by terminal columns, preserving newlines and supporting long unbroken errors.
pub(in crate::tui::ui) fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    for logical in text.split('\n') {
        let mut row = String::new();
        let mut used = 0;
        for c in logical.chars().filter(|c| !c.is_control()) {
            let size = Span::raw(c.to_string()).width();
            if used + size > width && !row.is_empty() {
                rows.push(row);
                row = String::new();
                used = 0;
            }
            row.push(c);
            used += size;
        }
        rows.push(row);
    }
    rows
}
