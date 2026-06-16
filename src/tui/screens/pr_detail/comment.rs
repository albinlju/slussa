use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    domain::{
        comment::{Comment, ReviewThread, split_suggestions},
        diff::{Diff, DiffLine},
    },
    tui::{format, markdown, theme, widgets},
};

pub(in crate::tui) fn render_inline_thread(
    thread: &ReviewThread,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    anchor_text: Option<&str>,
    author: &str,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let frame = if active { theme.accent } else { theme.divider };
    let anchor = thread.line.or(thread.old_line).zip(anchor_text);
    let has_suggestion = thread
        .comments
        .iter()
        .any(|c| !split_suggestions(&c.content).1.is_empty());

    // The code line is already in the diff, so no header/snippet — just the
    // conversation, rendered exactly like the Overview.
    let mut out: Vec<Line<'static>> = Vec::new();
    if !has_suggestion {
        out.push(header_line(Vec::new(), status_label(thread), width));
    }
    out.extend(conversation(thread, anchor, width, now, author, frame, ""));
    out
}

pub(super) fn comment_box(
    comment: &Comment,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    author: &str,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let frame = if active { theme.accent } else { theme.divider };
    let header = header_line(
        author_meta(
            &comment.author.username,
            comment_role(comment, author),
            comment.created,
            now,
        ),
        kind_label(comment),
        width,
    );
    let mut out = vec![header];
    out.extend(bracket(
        comment_body(comment, None, width.saturating_sub(2), ""),
        width,
        frame,
    ));
    out
}

/// Right-aligned header label: `SUGGESTION` when the comment proposes a change.
fn kind_label(comment: &Comment) -> Vec<Span<'static>> {
    let theme = theme::current();
    if split_suggestions(&comment.content).1.is_empty() {
        vec![Span::styled("COMMENT", Style::default().fg(theme.muted))]
    } else {
        vec![Span::styled(
            "SUGGESTION",
            Style::default()
                .fg(theme.suggestion)
                .add_modifier(Modifier::BOLD),
        )]
    }
}

pub(super) fn review_thread_box(
    thread: &ReviewThread,
    diff: Option<&Diff>,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    author: &str,
) -> Option<Vec<Line<'static>>> {
    let theme = theme::current();
    thread.comments.first()?;
    let frame = if active { theme.accent } else { theme.divider };

    let loc = match thread.line.or(thread.old_line) {
        Some(l) => format!("{}:{l}", thread.path),
        None if thread.path.is_empty() => "comment thread".to_string(),
        None => thread.path.clone(),
    };
    let has_suggestion = thread
        .comments
        .iter()
        .any(|comment| !split_suggestions(&comment.content).1.is_empty());

    let mut out: Vec<Line<'static>> = Vec::new();
    // A suggestion thread folds its location into the suggested-change border,
    // so it needs no separate header row.
    if !has_suggestion {
        out.push(header_line(
            vec![Span::styled(loc.clone(), Style::default().fg(theme.accent))],
            status_label(thread),
            width,
        ));
    }

    let (snippet, anchor_text) = diff
        .map(|d| diff_snippet(d, &thread.path, thread.line, thread.old_line, width.saturating_sub(2)))
        .unwrap_or_default();
    let anchor = thread.line.or(thread.old_line).zip(anchor_text.as_deref());

    if !has_suggestion && !snippet.is_empty() {
        out.extend(bracket(align_snippet(snippet), width, frame));
    }
    out.extend(conversation(thread, anchor, width, now, author, frame, &loc));
    Some(out)
}

/// A header row with left meta and a right-aligned label.
fn header_line(left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: u16) -> Line<'static> {
    Line::from(widgets::justify_between(left, right, width as usize))
}

/// An open-right bracket frame (`┌`/`│`/`└`) around `body`.
fn bracket(body: Vec<Line<'static>>, width: u16, color: Color) -> Vec<Line<'static>> {
    let style = Style::default().fg(color);
    let rule = |corner: &str| {
        Line::from(Span::styled(
            format!("{corner}{}", "─".repeat((width as usize).saturating_sub(1))),
            style,
        ))
    };
    let mut out = vec![rule("┌")];
    for line in body {
        let mut spans = vec![Span::styled("│ ", style)];
        spans.extend(line.spans);
        out.push(Line::from(spans).style(line.style));
    }
    out.push(rule("└"));
    out
}

/// Comments under a thread: a dotted `┊` trunk, but a comment that carries a
/// suggestion breaks out into its own framed box.
fn conversation(
    thread: &ReviewThread,
    anchor: Option<(usize, &str)>,
    width: u16,
    now: DateTime<Utc>,
    author: &str,
    frame: Color,
    loc: &str,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let style = Style::default().fg(theme.divider);
    let last = thread.comments.len().saturating_sub(1);
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, comment) in thread.comments.iter().enumerate() {
        if i > 0 {
            out.push(prefix_gutter(Line::raw(""), "┊ ", style));
        }
        let meta = author_meta(
            &comment.author.username,
            comment_role(comment, author),
            comment.created,
            now,
        );
        if !split_suggestions(&comment.content).1.is_empty() {
            let mut label = status_label(thread);
            label.push(Span::styled(
                "   SUGGESTION",
                Style::default().fg(theme.suggestion),
            ));
            out.extend(framed(
                meta,
                label,
                comment_body(comment, anchor, width.saturating_sub(2), loc),
                width,
                frame,
            ));
            continue;
        }
        let (head, body_gutter) = match i {
            0 => ("┊ ", "┊ "),
            n if n == last => ("└ ", "  "),
            _ => ("├ ", "┊ "),
        };
        out.push(prefix_gutter(Line::from(meta), head, style));
        for line in comment_body(comment, anchor, width.saturating_sub(2), "") {
            out.push(prefix_gutter(line, body_gutter, style));
        }
    }
    out
}

fn prefix_gutter(line: Line<'static>, gutter: &'static str, style: Style) -> Line<'static> {
    let mut spans = vec![Span::styled(gutter, style)];
    spans.extend(line.spans);
    Line::from(spans).style(line.style)
}

/// An open-right frame with the header embedded in the top rule:
/// `┌─ left ──────── right ─`, `│ body`, `└──────`.
fn framed(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    body: Vec<Line<'static>>,
    width: u16,
    color: Color,
) -> Vec<Line<'static>> {
    let style = Style::default().fg(color);
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
        let mut spans = vec![Span::styled("│ ", style)];
        spans.extend(line.spans);
        out.push(Line::from(spans).style(line.style));
    }
    out.push(Line::from(Span::styled(
        format!("└{}", "─".repeat(w.saturating_sub(1))),
        style,
    )));
    out
}

fn status_label(thread: &ReviewThread) -> Vec<Span<'static>> {
    let theme = theme::current();
    let (text, color) = if thread.resolved {
        ("resolved", theme.success)
    } else {
        ("unresolved", theme.warning)
    };
    vec![Span::styled(text, Style::default().fg(color))]
}

fn author_meta(
    name: &str,
    role: Option<&str>,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<Span<'static>> {
    let mut spans = author_title(name, role);
    spans.push(Span::styled(
        format!(" · {}", format::relative_age(created, now)),
        Style::default().fg(theme::current().muted),
    ));
    spans
}

/// Drops the diff gutter's leading space so code lines start in the same column
/// as the comment text below them.
fn align_snippet(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let style = line.style;
            let mut spans = line.spans;
            if let Some(first) = spans.first_mut()
                && let Some(trimmed) = first.content.strip_prefix(' ').map(str::to_owned)
            {
                *first = Span::styled(trimmed, first.style);
            }
            Line::from(spans).style(style)
        })
        .collect()
}

fn comment_role(comment: &Comment, author: &str) -> Option<&'static str> {
    if comment.author.username == author {
        Some(" · author")
    } else if !split_suggestions(&comment.content).1.is_empty() {
        Some(" · suggested a change")
    } else {
        None
    }
}

fn author_title(name: &str, note: Option<&str>) -> Vec<Span<'static>> {
    let theme = theme::current();
    let mut spans = vec![Span::styled(
        name.to_string(),
        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
    )];
    if let Some(note) = note {
        spans.push(Span::styled(
            note.to_string(),
            Style::default().fg(theme.muted),
        ));
    }
    spans
}

fn comment_body(
    comment: &Comment,
    anchor: Option<(usize, &str)>,
    text_w: u16,
    loc: &str,
) -> Vec<Line<'static>> {
    let (prose, suggestions) = split_suggestions(&comment.content);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if !prose.trim().is_empty() {
        lines.extend(paint_fg(markdown::render_no_margin(&prose, text_w)));
    }
    for suggestion in &suggestions {
        lines.push(Line::raw(""));
        lines.extend(suggestion_box(anchor, suggestion, text_w, loc));
    }
    if let Some(line) = widgets::reactions_line(&comment.reactions) {
        lines.push(Line::raw(""));
        lines.push(line);
    }
    lines
}

// Markdown (glamour) renders body text dimmer than the theme foreground; repaint
// each span's fg so comment prose reads in fg.
fn paint_fg(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let fg = theme::current().fg;
    lines
        .into_iter()
        .map(|line| {
            let style = line.style;
            let spans: Vec<Span<'static>> = line
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content, s.style.fg(fg)))
                .collect();
            Line::from(spans).style(style)
        })
        .collect()
}

fn suggestion_box(
    anchor: Option<(usize, &str)>,
    suggestion: &str,
    width: u16,
    loc: &str,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let text_w = (width as usize).saturating_sub(2);
    let new_lines: Vec<&str> = suggestion.lines().collect();

    let start = anchor.map(|(n, _)| n);
    let num_width = start.map_or(0, |n| {
        (n + new_lines.len().saturating_sub(1)).to_string().len()
    });
    let mut rows: Vec<Line<'static>> = Vec::new();
    if let Some((n, old)) = anchor {
        rows.push(widgets::numbered_diff_row(
            Some(n as u32),
            num_width,
            "-",
            old,
            theme.diff_removed,
            None,
            theme.diff_removed,
            text_w,
        ));
    }
    for (i, new) in new_lines.iter().enumerate() {
        rows.push(widgets::numbered_diff_row(
            start.map(|n| (n + i) as u32),
            num_width,
            "+",
            new,
            theme.diff_added,
            None,
            theme.diff_added,
            text_w,
        ));
    }

    let title = vec![Span::styled(
        "suggested change",
        Style::default()
            .fg(theme::current().fg)
            .add_modifier(Modifier::BOLD),
    )];
    let right = if loc.is_empty() {
        Vec::new()
    } else {
        vec![Span::styled(loc.to_string(), Style::default().fg(theme.accent))]
    };
    let mut out = framed(title, right, rows, width, theme.suggestion);
    out.push(Line::raw(""));
    out.push(suggestion_actions());
    out
}

fn suggestion_actions() -> Line<'static> {
    let theme = theme::current();
    let key = |k: &str| {
        Span::styled(
            k.to_string(),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )
    };
    let label = |l: &str| Span::styled(l.to_string(), Style::default().fg(theme.muted));
    Line::from(vec![
        key("[a]"),
        Span::raw(" "),
        label("apply suggestion"),
        Span::raw("   "),
        key("[b]"),
        Span::raw(" "),
        label("add to batch"),
    ])
}

const SNIPPET_CONTEXT: usize = 3;

fn diff_snippet(
    diff: &Diff,
    path: &str,
    line: Option<usize>,
    old_line: Option<usize>,
    width: u16,
) -> (Vec<Line<'static>>, Option<String>) {
    struct Row<'a> {
        dl: &'a DiffLine,
        new_no: usize,
        old_no: usize,
        hunk: usize,
    }

    let theme = theme::current();
    let Some(file) = diff.files.iter().find(|f| f.path == path) else {
        return (Vec::new(), None);
    };

    let mut rows: Vec<Row> = Vec::new();
    for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
        for (dl, new_no, old_no) in hunk.numbered_lines() {
            rows.push(Row {
                dl,
                new_no,
                old_no,
                hunk: hunk_idx,
            });
        }
    }

    let anchor = rows.iter().position(|r| match (line, old_line) {
        (Some(l), _) => !matches!(r.dl, DiffLine::Removed(_)) && r.new_no == l,
        (None, Some(o)) => matches!(r.dl, DiffLine::Removed(_)) && r.old_no == o,
        _ => false,
    });
    let Some(anchor) = anchor else {
        return (Vec::new(), None);
    };
    let anchor_text = rows[anchor].dl.content().to_string();

    let hunk_start = rows[..anchor]
        .iter()
        .rposition(|r| r.hunk != rows[anchor].hunk)
        .map_or(0, |i| i + 1);
    let start = anchor.saturating_sub(SNIPPET_CONTEXT).max(hunk_start);
    let window = &rows[start..=anchor];
    let num_width = window
        .iter()
        .map(|r| r.new_no)
        .max()
        .unwrap_or(1)
        .to_string()
        .len();
    let row_w = width as usize;

    let lines: Vec<Line<'static>> = window
        .iter()
        .map(|r| match r.dl {
            DiffLine::Added(c) => widgets::numbered_diff_row(
                Some(r.new_no as u32),
                num_width,
                "+",
                c,
                theme.diff_added,
                None,
                theme.diff_added,
                row_w,
            ),
            DiffLine::Removed(c) => widgets::numbered_diff_row(
                None,
                num_width,
                "-",
                c,
                theme.diff_removed,
                None,
                theme.diff_removed,
                row_w,
            ),
            DiffLine::Context(c) => widgets::numbered_diff_row(
                Some(r.new_no as u32),
                num_width,
                " ",
                c,
                theme.muted,
                None,
                theme.diff_context,
                row_w,
            ),
        })
        .collect();
    (lines, Some(anchor_text))
}
