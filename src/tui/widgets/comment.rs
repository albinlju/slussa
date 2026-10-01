use crate::{
    domain::{
        comment::{Comment, CommentThread, split_suggestions},
        diff::{Diff, DiffLine, LineRef},
    },
    tui::{
        format, theme,
        widgets::{self, markdown},
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// The diff line a thread is anchored to (new- or old-side), if any.
fn anchor_pos(thread: &CommentThread) -> Option<usize> {
    thread.anchor.as_ref()?.line.map(LineRef::number)
}

pub(in crate::tui) fn render_inline_thread(
    thread: &CommentThread,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    anchor_text: Option<&str>,
    author: &str,
    expanded: bool,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let frame = if active { theme.accent } else { theme.divider };

    // A resolved thread collapses to a one-line summary until expanded (`space`).
    if thread.resolved() && !expanded {
        return vec![collapse_summary(thread, false, active, width)];
    }

    let pos = anchor_pos(thread).zip(anchor_text);
    let has_suggestion = thread
        .comments
        .iter()
        .any(|c| !split_suggestions(&c.content).1.is_empty());

    let mut out: Vec<Line<'static>> = Vec::new();
    if thread.resolved() {
        out.push(collapse_summary(thread, true, active, width));
    } else if !has_suggestion {
        out.push(status_rule(status_label(thread.resolved()), width, frame));
    }
    out.extend(conversation(thread, pos, width, now, author, frame, None, false).0);
    out
}

/// Keep disclosure/count stable between states; secondary metadata yields first
/// in a narrow pane. No additional rows or background are needed for focus.
fn collapse_summary(
    thread: &CommentThread,
    expanded: bool,
    active: bool,
    width: u16,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }
    let theme = theme::current();
    let count = thread.comments.len();
    let label = if count == 1 {
        "1 comment".to_owned()
    } else {
        format!("{count} comments")
    };
    let focus = Style::default().fg(if active { theme.accent } else { theme.fg });
    let mut spans = vec![
        Span::styled(if expanded { "⌄ " } else { "› " }, focus),
        Span::styled(label, focus.add_modifier(Modifier::BOLD)),
        Span::styled(" · ", Style::default().fg(theme.divider)),
        Span::styled("✓ resolved", Style::default().fg(theme.success)),
    ];
    if let Some(comment) = thread.comments.first() {
        spans.push(Span::styled(
            format!(" · @{}", comment.author.username),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(widgets::truncate_to_width(spans, width as usize))
}

fn status_rule(label: Vec<Span<'static>>, width: u16, color: Color) -> Line<'static> {
    let style = Style::default().fg(color);
    let label_w: usize = label.iter().map(Span::width).sum();
    let fill = (width as usize).saturating_sub(label_w + 3).max(1);
    let mut spans = vec![Span::styled(format!("{} ", "─".repeat(fill)), style)];
    spans.extend(label);
    spans.push(Span::styled(" ─", style));
    Line::from(spans)
}

pub(in crate::tui) fn comment_box(
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
        comment_body(comment, None, width.saturating_sub(2)),
        width,
        frame,
        theme.divider,
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

pub(in crate::tui) fn comment_thread_box(
    thread: &CommentThread,
    diff: Option<&Diff>,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    selected: Option<usize>,
    author: &str,
) -> Option<(Vec<Line<'static>>, Option<std::ops::Range<usize>>)> {
    let diff = diff.filter(|d| thread.matches_revision(d.revision.as_ref()));
    let theme = theme::current();
    let first = thread.comments.first()?;
    let frame = if active { theme.accent } else { theme.divider };

    let location = thread.anchor.as_ref().and_then(|a| match a.line {
        Some(l) => Some(format!("{}:{}", a.path, l.number())),
        None if !a.path.is_empty() => Some(a.path.clone()),
        None => None,
    });
    let has_suggestion = thread
        .comments
        .iter()
        .any(|comment| !split_suggestions(&comment.content).1.is_empty());

    // The header reads as the opening activity: "@user commented on file:line".
    let action = if split_suggestions(&first.content).1.is_empty() {
        "commented"
    } else {
        "suggested a change"
    };
    let phrase = if location.is_some() {
        format!(" {action} on ")
    } else {
        format!(" {action}")
    };
    let mut left: Vec<Span<'static>> = vec![Span::styled(
        format!("@{}", first.author.username),
        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
    )];
    left.push(Span::styled(phrase, Style::default().fg(theme.muted)));
    if let Some(location) = location {
        left.push(Span::styled(location, Style::default().fg(theme.link)));
    }
    let age = format::relative_age(first.created, now);
    // A general discussion thread (no anchor) has no resolve status — just the
    // age; review threads keep their resolved/unresolved label.
    let right: Vec<Span<'static>> = if thread.anchor.is_none() {
        vec![Span::styled(age, Style::default().fg(theme.muted))]
    } else {
        let mut r = status_label(thread.resolved());
        r.push(Span::styled(
            format!("  ·  {age}"),
            Style::default().fg(theme.muted),
        ));
        r
    };
    let mut out: Vec<Line<'static>> = vec![header_line(left, right, width)];

    let (snippet, anchor_text) = match (diff, &thread.anchor) {
        (Some(d), Some(a)) => diff_snippet(d, &a.path, a.line, width.saturating_sub(2)),
        _ => Default::default(),
    };
    let pos = anchor_pos(thread).zip(anchor_text.as_deref());

    if !has_suggestion && !snippet.is_empty() {
        out.extend(bracket(align_snippet(snippet), width, frame, theme.divider));
    }
    // The header carries the first comment's author + time, so the conversation
    // skips its meta line to avoid repeating it.
    let offset = out.len();
    let (lines, selected_range) =
        conversation(thread, pos, width, now, author, frame, selected, true);
    out.extend(lines);
    let selected_range = selected_range.map(|range| {
        if selected == Some(0) {
            0..offset + range.end
        } else {
            offset + range.start..offset + range.end
        }
    });
    Some((out, selected_range))
}

fn header_line(left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: u16) -> Line<'static> {
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
fn bracket(body: Vec<Line<'static>>, width: u16, left: Color, rule: Color) -> Vec<Line<'static>> {
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

#[expect(clippy::too_many_arguments, reason = "render inputs; see ROADMAP")]
fn conversation(
    thread: &CommentThread,
    anchor: Option<(usize, &str)>,
    width: u16,
    now: DateTime<Utc>,
    author: &str,
    frame: Color,
    selected: Option<usize>,
    skip_first_meta: bool,
) -> (Vec<Line<'static>>, Option<std::ops::Range<usize>>) {
    let theme = theme::current();
    let style = Style::default().fg(frame);
    let last = thread.comments.len().saturating_sub(1);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut selected_range = None;
    for (i, comment) in thread.comments.iter().enumerate() {
        // The first comment's author/time live in the box header when requested.
        let suppress = skip_first_meta && i == 0;
        if i > 0 {
            out.push(prefix_gutter(Line::raw(""), "┊ ", style));
        }
        let start = out.len();
        let meta = if suppress {
            Vec::new()
        } else {
            author_meta(
                &comment.author.username,
                comment_role(comment, author),
                comment.created,
                now,
            )
        };
        if !split_suggestions(&comment.content).1.is_empty() {
            let label = if suppress {
                Vec::new()
            } else {
                status_label(thread.resolved())
            };
            out.extend(framed(
                meta,
                label,
                comment_body(comment, anchor, width.saturating_sub(2)),
                width,
                frame,
                theme.divider,
            ));
            if selected == Some(i) {
                selected_range = Some(start..out.len());
            }
            continue;
        }
        let (head, body_gutter) = match i {
            0 => ("┊ ", "┊ "),
            n if n == last => ("└ ", "  "),
            _ => ("├ ", "┊ "),
        };
        // The Ctrl-j/k sub-cursor marks the comment e/d will act on.
        if suppress {
            // Header already shows the meta; the body hangs straight off the trunk.
        } else if selected == Some(i) {
            let marker = Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD);
            let mut spans = vec![Span::styled("▸ ", marker)];
            spans.extend(meta);
            out.push(Line::from(spans));
        } else {
            out.push(prefix_gutter(Line::from(meta), head, style));
        }
        for line in comment_body(comment, anchor, width.saturating_sub(2)) {
            out.push(prefix_gutter(line, body_gutter, style));
        }
        if selected == Some(i) {
            selected_range = Some(start..out.len());
        }
    }
    (out, selected_range)
}

fn prefix_gutter(line: Line<'static>, gutter: &'static str, style: Style) -> Line<'static> {
    let mut spans = vec![Span::styled(gutter, style)];
    spans.extend(line.spans);
    Line::from(spans).style(line.style)
}

fn framed(
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

fn status_label(resolved: bool) -> Vec<Span<'static>> {
    let theme = theme::current();
    let (text, color) = if resolved {
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
) -> Vec<Line<'static>> {
    let (prose, suggestions) = split_suggestions(&comment.content);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if !prose.trim().is_empty() {
        lines.extend(paint_fg(markdown::render_no_margin(&prose, text_w)));
    }
    for suggestion in &suggestions {
        lines.push(Line::raw(""));
        lines.extend(suggestion_box(anchor, suggestion, text_w));
    }
    if let Some(line) = widgets::reactions_line(&comment.reactions) {
        lines.push(Line::raw(""));
        lines.push(line);
    }
    lines
}

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
            u32::try_from(n).ok(),
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
            start.and_then(|n| u32::try_from(n + i).ok()),
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
    framed(
        title,
        Vec::new(),
        rows,
        width,
        theme.suggestion,
        theme.suggestion,
    )
}

const SNIPPET_CONTEXT: usize = 3;

fn diff_snippet(
    diff: &Diff,
    path: &str,
    line: Option<LineRef>,
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

    let mut rows: Vec<Row<'_>> = Vec::new();
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

    let anchor = rows.iter().position(|r| match line {
        Some(LineRef::New(l)) => !matches!(r.dl, DiffLine::Removed(_)) && r.new_no == l,
        Some(LineRef::Old(o)) => matches!(r.dl, DiffLine::Removed(_)) && r.old_no == o,
        None => false,
    });
    let Some((anchor, anchor_row)) = anchor.and_then(|i| Some((i, rows.get(i)?))) else {
        return (Vec::new(), None);
    };
    let anchor_text = anchor_row.dl.content().to_string();

    let hunk_start = (0..anchor)
        .rev()
        .find(|&i| rows.get(i).is_some_and(|r| r.hunk != anchor_row.hunk))
        .map_or(0, |i| i + 1);
    let start = anchor.saturating_sub(SNIPPET_CONTEXT).max(hunk_start);
    let window = rows.get(start..=anchor).unwrap_or_default();
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
                u32::try_from(r.new_no).ok(),
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
                u32::try_from(r.new_no).ok(),
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

#[cfg(test)]
mod tests {
    use super::*;

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
