use chrono::{DateTime, Utc};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::domain::comment::Reaction;
use crate::tui::theme;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// One-line reaction pills under a comment, powerline-capped like the
/// header's status badge. The user's own reactions sit on a dark accent
/// tint with an accent count — a full accent fill would drown yellow emojis.
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

/// A `/` search prompt line: `Search: query█            N match`. Shared by
/// every searchable view's footer so they all look and read the same.
pub(super) fn search_prompt(query: &str, count: usize, width: u16) -> Line<'static> {
    let theme = theme::current();
    let left = vec![
        Span::styled(format!("  Search: {query}"), Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ];
    let right = vec![Span::styled(
        format!("{count} match  "),
        Style::default().fg(theme.muted),
    )];
    Line::from(justify_between(left, right, width as usize))
}

/// Re-style every case-insensitive occurrence of `query` inside `line` with
/// `match_style` (patched onto each span's own style, so diff tints read
/// through). Spans that change byte length when lowercased (non-ASCII) are
/// left untouched to keep slicing on char boundaries.
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

/// `left … pad … right` spans filling `width`, with at least one space
/// between the groups.
pub(super) fn justify_between(
    mut left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: usize,
) -> Vec<Span<'static>> {
    let left_w: usize = left.iter().map(|s| s.width()).sum();
    let right_w: usize = right.iter().map(|s| s.width()).sum();
    let pad = width.saturating_sub(left_w + right_w).max(1);
    left.push(Span::raw(" ".repeat(pad)));
    left.extend(right);
    left
}

/// Fixed 3-row thumb that slides the full track, instead of ratatui's
/// proportional `Scrollbar` whose thumb barely moves on short scroll ranges.
/// The thumb compresses against the bottom at the end of the track.
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

/// Rightmost-column slice of `area` — where the vertical scrollbar lives.
pub(super) fn scrollbar_area(area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    }
}

/// Markdown → ratatui lines via charmed-glamour, wrapped to `width`. A
/// glamour panic or ANSI-bridge failure falls back to the raw body instead
/// of taking down the TUI.
pub(super) fn markdown(body: &str, width: u16) -> Vec<Line<'static>> {
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

pub(super) fn trim_blank_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
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

/// Peel off glamour's fixed-width document margin (`n` leading chars per
/// line) so the rendered markdown sits flush against whatever frames it.
pub(super) fn strip_glamour_margin(lines: Vec<Line<'static>>, n: usize) -> Vec<Line<'static>> {
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
    let visible: usize = line.spans.iter().map(|s| s.width()).sum();
    let pad = text_w.saturating_sub(visible);
    let line_style = line.style;
    // Extend an edge span's bg tint into the 1-col gap so the fill reaches
    // the inner borders.
    let leading_bg = line.spans.first().and_then(|s| s.style.bg);
    let trailing_bg = line.spans.last().and_then(|s| s.style.bg);
    let pad_style = |bg: Option<ratatui::style::Color>| match bg {
        Some(c) => Style::default().bg(c),
        None => Style::default(),
    };
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 4);
    spans.push(Span::styled("│", border));
    spans.push(Span::styled(" ", pad_style(leading_bg)));
    for s in line.spans {
        let merged = line_style.patch(s.style);
        spans.push(Span::styled(s.content, merged));
    }
    spans.push(Span::styled(" ".repeat(pad + 1), pad_style(trailing_bg)));
    spans.push(Span::styled("│", border));
    Line::from(spans)
}

/// A line-numbered diff row: `{num} {prefix} {content}`, padded to `row_w`
/// so an optional bg tint fills the whole row.
#[allow(clippy::too_many_arguments)]
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
    let visible = gutter.chars().count() + prefix.chars().count() + 1 + content.chars().count();
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
    let visible = gutter.chars().count() + prefix.chars().count() + content.chars().count();
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

pub(super) fn relative_age(when: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let delta = now - when;
    let days = delta.num_days();
    if days >= 1 {
        format!("{days}d ago")
    } else {
        let hours = delta.num_hours();
        if hours >= 1 {
            format!("{hours}h ago")
        } else {
            "just now".to_string()
        }
    }
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
        // Spans per pill: cap, label, cap (+ a gap span between pills).
        assert_eq!(line.spans[1].style.bg, Some(theme.accent_bg));
        assert_eq!(line.spans[1].style.fg, Some(theme.accent));
        assert_eq!(line.spans[5].style.bg, Some(theme.highlight_bg));
    }
}
