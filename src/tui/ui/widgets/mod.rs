pub mod comment;
mod comment_code;
pub mod comment_fold;
mod comment_frame;
pub mod comment_meta;
pub mod dialog;
pub mod markdown;
pub mod table;
use crate::{
    domain::comment::Reaction,
    tui::app::store::LoadState,
    tui::ui::{component::saturating_u16, format, layout, theme},
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

pub(super) fn author_line(
    mut lead: Vec<Span<'static>>,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Line<'static> {
    lead.push(Span::styled(
        format!(" · {}", format::relative_age(created, now)),
        Style::default().fg(theme::current().muted),
    ));
    Line::from(lead)
}

pub(super) fn framed_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    focused: bool,
) -> (Rect, Rect) {
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {title} "))
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.divider }));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [band_area, body_area] = layout::split(
        inner,
        Direction::Vertical,
        [
            Constraint::Length(if area.height < 12 { 1 } else { 2 }),
            Constraint::Min(0),
        ],
    );

    let header_band = Block::default()
        .borders(if area.height < 12 {
            Borders::NONE
        } else {
            Borders::BOTTOM
        })
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_band.inner(band_area);
    frame.render_widget(header_band, band_area);

    (header_inner, body_area)
}

pub(super) fn empty_state(text: &str) -> Paragraph<'static> {
    Paragraph::new(text.to_string())
        .style(Style::default().fg(theme::current().muted))
        .wrap(Wrap { trim: false })
}

pub(super) fn loaded_or_placeholder<'a, T>(
    frame: &mut Frame<'_>,
    state: Option<&'a LoadState<T>>,
    noun: &str,
    area: Rect,
) -> Option<&'a T> {
    let theme = theme::current();
    match state {
        Some(LoadState::Loaded(value)) => return Some(value),
        Some(LoadState::Failed(error)) => {
            let message = error.user_message();
            let p = Paragraph::new(format!("Couldn't load {noun}. F: retry\n{message}"))
                .style(Style::default().fg(theme.error))
                .wrap(Wrap { trim: false });
            frame.render_widget(p, area);
        }
        Some(LoadState::Loading) => {
            frame.render_widget(Paragraph::new(loading(&format!("Loading {noun}..."))), area);
        }
        None | Some(LoadState::NotRequested) => {
            frame.render_widget(empty_state(&format!("No {noun} loaded. F: refresh")), area);
        }
    }
    None
}

pub(super) fn scrolled_paragraph(
    frame: &mut Frame<'_>,
    lines: Vec<Line<'static>>,
    scroll: &mut u16,
    viewport: &mut u16,
    area: Rect,
) {
    let max_scroll = saturating_u16(lines.len().saturating_sub(area.height as usize));
    *scroll = (*scroll).min(max_scroll);
    *viewport = area.height;

    let content_area = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), content_area);

    if max_scroll > 0 {
        let bar = scrollbar(*scroll, max_scroll, area.height);
        frame.render_widget(Paragraph::new(bar), layout::scrollbar_area(area));
    }
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(super) fn loading(text: &str) -> Line<'static> {
    Line::styled(
        format!("{}  {text}", spinner_frame()),
        Style::default().fg(theme::current().warning),
    )
}

pub(super) fn reactions_line(reactions: &[Reaction]) -> Option<Line<'static>> {
    if reactions.is_empty() {
        return None;
    }
    let theme = theme::current();
    let mut spans: Vec<Span<'static>> = Vec::new();
    for r in reactions {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        let fg = if r.mine {
            theme.reaction_mine
        } else {
            theme.fg
        };
        spans.push(Span::styled(
            format!(" {} {} ", r.emoji, r.count),
            Style::default().fg(fg).bg(theme.highlight_bg),
        ));
    }
    Some(Line::from(spans))
}

pub(super) fn spinner_frame() -> &'static str {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = (now / 100) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES.get(idx).copied().unwrap_or(" ")
}

/// A footer action hint. `enabled == false` renders it dimmed (key not accented)
/// — the "disabled with affordance" pattern: the action stays visible with its
/// reason instead of being hidden.
pub(super) struct Hint {
    text: String,
    enabled: bool,
}

impl Hint {
    pub(super) fn on(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: true,
        }
    }

    pub(super) fn off(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: false,
        }
    }
}

/// Build a row of all-enabled hints from a `"a: x  b: y"` string.
pub(super) fn hints_on(s: &str) -> Vec<Hint> {
    s.split("  ").map(Hint::on).collect()
}

pub(super) fn footer(width: u16, hints: &[Hint], refreshing: bool) -> Line<'static> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);
    let key = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);

    let width = width as usize;
    let right = if refreshing && width >= 30 {
        "refreshing…  ?: help "
    } else {
        "?: help "
    };
    let right = truncate_to_width(vec![Span::styled(right, muted)], width);
    let right_width: usize = right.iter().map(Span::width).sum();
    let budget = width.saturating_sub(right_width + 1);
    let mut left = Vec::new();
    let mut used = 0;
    for hint in hints {
        let hint_width = Span::raw(&hint.text).width() + 2;
        if used + hint_width > budget {
            continue;
        }
        left.push(Span::raw("  "));
        let key_style = if hint.enabled { key } else { muted };
        match hint.text.split_once(": ") {
            Some((keys, desc)) => {
                left.push(Span::styled(keys.to_string(), key_style));
                left.push(Span::styled(format!(": {desc}"), muted));
            }
            None => left.push(Span::styled(hint.text.clone(), muted)),
        }
        used += hint_width;
    }
    left.push(Span::raw(
        " ".repeat(width.saturating_sub(used + right_width)),
    ));
    left.extend(right);
    Line::from(left)
}

pub(super) fn search_input_spans(query: &str) -> Vec<Span<'static>> {
    let theme = theme::current();
    vec![
        Span::styled(format!("  Search: {query}"), Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ]
}

pub(super) fn search_prompt(query: &str, count: usize, width: u16) -> Line<'static> {
    let theme = theme::current();
    let right = vec![Span::styled(
        format!("{count} match  "),
        Style::default().fg(theme.muted),
    )];
    Line::from(justify_between(
        search_input_spans(query),
        right,
        width as usize,
    ))
}

pub(super) fn highlight_query(
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
pub(super) fn fitted_row(
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

pub(super) fn justify_between(
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

pub(super) fn scrollbar(scroll: u16, max_scroll: u16, height: u16) -> Vec<Line<'static>> {
    if max_scroll == 0 || height == 0 {
        return Vec::new();
    }
    let theme = theme::current();
    let track_len = height as usize;
    let thumb_size = 3usize.min(track_len);
    let max_thumb_top = track_len.saturating_sub(1);
    let thumb_top = (scroll as usize * max_thumb_top) / max_scroll as usize;

    (0..track_len)
        .map(|y| {
            if y >= thumb_top && y < thumb_top + thumb_size {
                Line::styled("█", Style::default().fg(theme.accent))
            } else {
                Line::styled("│", Style::default().fg(theme.muted))
            }
        })
        .collect()
}

pub(super) fn truncate_to_width(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
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

#[expect(
    clippy::too_many_arguments,
    reason = "one styled diff row — splitting the args adds no clarity"
)]
pub(super) fn numbered_diff_row(
    line_num: Option<u32>,
    num_width: usize,
    prefix: &'static str,
    content: &str,
    prefix_fg: Color,
    bg: Option<Color>,
    text_fg: Color,
    row_w: usize,
) -> Line<'static> {
    let num_str = match line_num {
        Some(n) => format!("{n:>num_width$}"),
        None => " ".repeat(num_width),
    };
    let gutter = format!(" {num_str} ");
    let visible = gutter.len() + prefix.len() + 1 + Span::raw(content).width();
    let pad = row_w.saturating_sub(visible);

    let apply_bg = |style: Style| match bg {
        Some(bg) => style.bg(bg),
        None => style,
    };
    let gutter_style = apply_bg(Style::default().fg(theme::current().muted));
    let prefix_style = apply_bg(Style::default().fg(prefix_fg).add_modifier(Modifier::BOLD));
    let text_style = apply_bg(Style::default().fg(text_fg));

    Line::from(vec![
        Span::styled(gutter, gutter_style),
        Span::styled(prefix, prefix_style),
        Span::styled(format!(" {content}{}", " ".repeat(pad)), text_style),
    ])
}

/// Wrap by terminal columns, preserving newlines and supporting long unbroken errors.
pub(super) fn wrap_text(text: &str, width: usize) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reaction(emoji: &str, count: u32, mine: bool) -> Reaction {
        Reaction {
            emoji: emoji.to_string(),
            count,
            mine,
        }
    }

    fn text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn renders_one_padded_pill_per_reaction() {
        let line = reactions_line(&[reaction("👍", 2, false), reaction("👀", 3, false)]).unwrap();
        assert_eq!(text(&line), " 👍 2   👀 3 ");
    }

    #[test]
    fn own_reaction_gets_the_accent_text() {
        let theme = theme::current();
        let line = reactions_line(&[reaction("👍", 4, true), reaction("👀", 3, false)]).unwrap();
        assert_eq!(line.spans[0].style.bg, Some(theme.highlight_bg));
        assert_eq!(line.spans[0].style.fg, Some(theme.reaction_mine));
        assert_eq!(line.spans[2].style.fg, Some(theme.fg));
    }
}
