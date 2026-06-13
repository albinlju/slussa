use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::state::LoadState;
use crate::domain::comment::Reaction;
use crate::tui::{format, theme};

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

pub(super) fn framed_panel(frame: &mut Frame, area: Rect, focused: bool) -> (Rect, Rect) {
    let theme = theme::current();
    let border = if focused { theme.accent } else { theme.divider };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(inner);

    let header_band = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_band.inner(chunks[0]);
    frame.render_widget(header_band, chunks[0]);

    (header_inner, chunks[1])
}

pub(super) fn empty_state(text: &str) -> Paragraph<'static> {
    Paragraph::new(text.to_string()).style(Style::default().fg(theme::current().muted))
}

pub(super) fn loaded_or_placeholder<'a, T>(
    frame: &mut Frame,
    state: Option<&'a LoadState<T>>,
    noun: &str,
    area: Rect,
) -> Option<&'a T> {
    let theme = theme::current();
    match state {
        Some(LoadState::Loaded(value)) => return Some(value),
        Some(LoadState::Failed(msg)) => {
            let p = Paragraph::new(format!("Couldn't load {noun}: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(p, area);
        }
        None | Some(LoadState::NotRequested | LoadState::Loading) => {
            frame.render_widget(Paragraph::new(loading(&format!("Loading {noun}..."))), area);
        }
    }
    None
}

pub(super) fn scrolled_paragraph(
    frame: &mut Frame,
    lines: Vec<Line<'static>>,
    scroll: &mut u16,
    viewport: &mut u16,
    area: Rect,
) {
    let max_scroll = lines.len().saturating_sub(area.height as usize) as u16;
    *scroll = (*scroll).min(max_scroll);
    *viewport = area.height;

    let content_area = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), content_area);

    if max_scroll > 0 {
        let bar = scrollbar(*scroll, max_scroll, area.height);
        frame.render_widget(Paragraph::new(bar), scrollbar_area(area));
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
        let (bg, fg) = if r.mine {
            (theme.accent_bg, theme.accent)
        } else {
            (theme.highlight_bg, theme.fg)
        };
        spans.push(Span::styled("\u{e0b6}", Style::default().fg(bg)));
        spans.push(Span::styled(
            format!("{} {}", r.emoji, r.count),
            Style::default().fg(fg).bg(bg),
        ));
        spans.push(Span::styled("\u{e0b4}", Style::default().fg(bg)));
    }
    Some(Line::from(spans))
}

pub(super) fn spinner_frame() -> &'static str {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = (now / 100) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

pub(super) fn footer(width: u16, hints: &str) -> Line<'static> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);
    let left = vec![Span::styled(format!("  {hints}"), muted)];
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // Nerd Font glyphs: \u{f004} heart, \u{f059} question-circle.
    let right = vec![
        Span::styled("\u{f004}", Style::default().fg(theme.orange)),
        Span::styled(" donate", muted),
        Span::raw("    "),
        Span::styled("\u{f059}", muted),
        Span::styled(" help", muted),
        Span::raw("    "),
        Span::styled(version, muted),
        Span::raw("  "),
    ];
    Line::from(justify_between(left, right, width as usize))
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

pub(super) fn scrollbar_area(area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    }
}

pub(super) fn box_text_width(outer: u16) -> u16 {
    outer.saturating_sub(4)
}

pub(super) fn boxed(
    header: Line<'static>,
    body: Vec<Line<'static>>,
    width: u16,
    border: Color,
) -> Vec<Line<'static>> {
    let style = Style::default().fg(border);
    let inner = (width as usize).saturating_sub(2);
    let text_w = inner.saturating_sub(2);

    let bar = |s: String| Line::from(Span::styled(s, style));

    let mut out: Vec<Line<'static>> = Vec::new();
    out.push(bar(format!("╭{}╮", "─".repeat(inner))));
    out.push(wrap_box_line(header, text_w, style));
    out.push(bar(format!("├{}┤", "─".repeat(inner))));
    for line in body {
        out.push(wrap_box_line(line, text_w, style));
    }
    out.push(bar(format!("╰{}╯", "─".repeat(inner))));
    out
}

fn wrap_box_line(line: Line<'static>, text_w: usize, border: Style) -> Line<'static> {
    let line_style = line.style;
    let content = truncate_to_width(line.spans, text_w);
    let visible: usize = content.iter().map(Span::width).sum();
    let pad = text_w.saturating_sub(visible);
    let leading_bg = content.first().and_then(|s| s.style.bg);
    let trailing_bg = content.last().and_then(|s| s.style.bg);
    let pad_style = |bg: Option<ratatui::style::Color>| match bg {
        Some(c) => Style::default().bg(c),
        None => Style::default(),
    };
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(content.len() + 4);
    spans.push(Span::styled("│", border));
    spans.push(Span::styled(" ", pad_style(leading_bg)));
    for s in content {
        let merged = line_style.patch(s.style);
        spans.push(Span::styled(s.content, merged));
    }
    spans.push(Span::styled(" ".repeat(pad + 1), pad_style(trailing_bg)));
    spans.push(Span::styled("│", border));
    Line::from(spans)
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

#[expect(clippy::too_many_arguments, reason = "one styled diff row — splitting the args adds no clarity")]
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

    let gutter_style = match bg {
        Some(bg) => Style::default().fg(theme::current().muted).bg(bg),
        None => Style::default().fg(theme::current().muted),
    };
    let prefix_style = {
        let s = Style::default()
            .fg(prefix_fg)
            .add_modifier(Modifier::BOLD);
        match bg {
            Some(bg) => s.bg(bg),
            None => s,
        }
    };
    let text_style = match bg {
        Some(bg) => Style::default().fg(text_fg).bg(bg),
        None => Style::default().fg(text_fg),
    };

    Line::from(vec![
        Span::styled(gutter, gutter_style),
        Span::styled(prefix, prefix_style),
        Span::styled(format!(" {content}{}", " ".repeat(pad)), text_style),
    ])
}

pub(super) fn diff_bg_row(
    gutter: &'static str,
    prefix: &'static str,
    content: &str,
    prefix_fg: Color,
    bg: Color,
    text_fg: Color,
    row_w: usize,
) -> Line<'static> {
    let visible = gutter.len() + prefix.len() + Span::raw(content).width();
    let pad = row_w.saturating_sub(visible);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(3);
    if !gutter.is_empty() {
        spans.push(Span::styled(gutter, Style::default().bg(bg)));
    }
    spans.push(Span::styled(
        prefix,
        Style::default()
            .fg(prefix_fg)
            .bg(bg)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        format!("{content}{}", " ".repeat(pad)),
        Style::default().fg(text_fg).bg(bg),
    ));
    Line::from(spans)
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
    fn renders_one_capped_pill_per_reaction() {
        let line = reactions_line(&[reaction("👍", 2, false), reaction("👀", 3, false)]).unwrap();
        assert_eq!(text(&line), "\u{e0b6}👍 2\u{e0b4} \u{e0b6}👀 3\u{e0b4}");
    }

    #[test]
    fn own_reaction_gets_the_accent_tint() {
        let theme = theme::current();
        let line = reactions_line(&[reaction("👍", 4, true), reaction("👀", 3, false)]).unwrap();
        assert_eq!(line.spans[1].style.bg, Some(theme.accent_bg));
        assert_eq!(line.spans[1].style.fg, Some(theme.accent));
        assert_eq!(line.spans[5].style.bg, Some(theme.highlight_bg));
    }
}
