use chrono::{DateTime, Utc};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::tui::theme;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

/// Build a `left ... pad ... right` span sequence that fills `width`. Widths
/// are measured with `Span::width()` so Nerd Font glyphs land where they
/// should. If left + right already exceed `width`, a single space keeps
/// them visually separated.
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

/// Vertical thumb-style scrollbar with a fixed 3-row thumb that slides from
/// the top to `height - 1` as `scroll` runs from `0` to `max_scroll`.
///
/// ratatui's built-in `Scrollbar` sizes its thumb proportionally to visible
/// content, which for short scroll ranges means the thumb's *top* only
/// inches down even at max scroll — it doesn't feel like "at the bottom".
/// A fixed thumb fixes that; it compresses to 1-2 rows at the very end
/// (lower rows fall outside the track), a small visual cost for clear
/// "I'm at the bottom" feedback.
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

/// Markdown body → ratatui lines via charmed-glamour, already wrapped to
/// `width`. Wraps the call in `catch_unwind` so a glamour panic falls back
/// to raw body lines instead of taking down the TUI.
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
        // ANSI bridge failed, or glamour panicked: fall back to the raw body.
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

/// Strip `n` leading chars from each line's first span — peels off
/// glamour's fixed-width document margin so the rendered markdown sits
/// flush against whatever frames it.
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
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let style = Style::default().fg(theme.divider);
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
    // If the line's leading/trailing span carries a bg tint, extend it into
    // the 1-col gap spaces so the fill reaches the inner borders. Otherwise
    // the gap is plain.
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
