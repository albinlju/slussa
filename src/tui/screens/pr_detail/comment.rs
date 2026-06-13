use chrono::{DateTime, Utc};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    domain::{
        comment::{Comment, ReviewThread, split_suggestions},
        diff::{Diff, DiffLine},
    },
    tui::{markdown, theme, widgets},
};

fn author_line(name: &str, note: Option<&str>, created: DateTime<Utc>, now: DateTime<Utc>) -> Line<'static> {
    let theme = theme::current();
    let mut lead = vec![Span::styled(
        name.to_string(),
        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
    )];
    if let Some(note) = note {
        lead.push(Span::styled(note.to_string(), Style::default().fg(theme.muted)));
    }
    widgets::author_line(lead, created, now)
}

fn comment_lines(
    name: &str,
    note: Option<&str>,
    comment: &Comment,
    anchor: Option<(usize, &str)>,
    width: u16,
    now: DateTime<Utc>,
) -> Vec<Line<'static>> {
    let mut lines = vec![author_line(name, note, comment.created, now)];
    lines.extend(comment_body(comment, anchor, width));
    lines
}

pub(in crate::tui) fn render_inline_thread(
    thread: &ReviewThread,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    anchor_text: Option<&str>,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let text_w = widgets::box_text_width(width);
    let border = if active { theme.accent } else { theme.divider };

    let (icon, label, accent) = if thread.resolved {
        ("\u{f058}", "Resolved conversation", theme.success) //  check-circle
    } else {
        ("\u{f071}", "Unresolved", theme.warning) //  exclamation-triangle
    };
    let count = thread.comments.len();
    let count_label = if count == 1 {
        "1 comment".to_string()
    } else {
        format!("{count} comments")
    };

    let left = vec![
        Span::styled(
            icon,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            label,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ];
    let right = vec![Span::styled(count_label, Style::default().fg(theme.muted))];
    let header = Line::from(widgets::justify_between(left, right, text_w as usize));

    let anchor = thread.line.or(thread.old_line).zip(anchor_text);
    let mut body: Vec<Line<'static>> = Vec::new();
    for (i, comment) in thread.comments.iter().enumerate() {
        if i > 0 {
            body.push(Line::raw(""));
        }
        body.extend(comment_lines(
            &comment.author.username,
            None,
            comment,
            anchor,
            width,
            now,
        ));
    }

    widgets::boxed(header, body, width, border)
}

pub(super) fn comment_box(
    comment: &Comment,
    width: u16,
    now: DateTime<Utc>,
) -> Vec<Line<'static>> {
    let header = author_line(&comment.author.username, Some(" commented"), comment.created, now);
    let body = comment_body(comment, None, width);
    widgets::boxed(header, body, width, theme::current().divider)
}

pub(super) fn review_thread_box(
    thread: &ReviewThread,
    diff: Option<&Diff>,
    width: u16,
    now: DateTime<Utc>,
) -> Option<Vec<Line<'static>>> {
    let theme = theme::current();
    let first = thread.comments.first()?;
    let text_width = widgets::box_text_width(width);
    let header = author_line(&first.author.username, Some(" commented"), first.created, now);

    let mut body: Vec<Line<'static>> = Vec::new();

    let location = match thread.line.or(thread.old_line) {
        Some(l) => format!("{}:{}", thread.path, l),
        None => thread.path.clone(),
    };
    let mut anchor_spans = vec![Span::styled(location, Style::default().fg(theme.accent))];
    if thread.resolved {
        anchor_spans.push(Span::styled(
            " · resolved",
            Style::default().fg(theme.success),
        ));
    }
    let inner_header = Line::from(anchor_spans);

    let inner_text_width = widgets::box_text_width(text_width);
    let (snippet, anchor_text) = diff
        .map(|d| diff_snippet(d, &thread.path, thread.line, thread.old_line, inner_text_width))
        .unwrap_or_default();
    // A suggestion repeats the anchored line, so the snippet box is dropped
    // when any comment carries one (to avoid showing the line twice).
    let has_suggestion = thread
        .comments
        .iter()
        .any(|comment| !split_suggestions(&comment.content).1.is_empty());
    if snippet.is_empty() || has_suggestion {
        body.push(inner_header);
    } else {
        body.extend(widgets::boxed(inner_header, snippet, text_width, theme.divider));
    }
    let anchor = thread.line.or(thread.old_line).zip(anchor_text.as_deref());

    for (i, comment) in thread.comments.iter().enumerate() {
        if i == 0 {
            body.extend(comment_body(comment, anchor, width));
        } else {
            body.push(Line::raw(""));
            body.extend(comment_lines(
                &format!("↳ @{}", comment.author.username),
                None,
                comment,
                anchor,
                width,
                now,
            ));
        }
    }

    Some(widgets::boxed(header, body, width, theme.divider))
}

fn comment_body(comment: &Comment, anchor: Option<(usize, &str)>, width: u16) -> Vec<Line<'static>> {
    let text_w = widgets::box_text_width(width);
    let (prose, suggestions) = split_suggestions(&comment.content);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if !prose.trim().is_empty() {
        lines.extend(markdown::render_no_margin(&prose, text_w));
    }
    for suggestion in &suggestions {
        lines.push(Line::raw(""));
        lines.extend(suggestion_lines(anchor, suggestion, text_w));
    }
    if let Some(line) = widgets::reactions_line(&comment.reactions) {
        lines.push(Line::raw(""));
        lines.push(line);
    }
    lines
}

fn suggestion_lines(
    anchor: Option<(usize, &str)>,
    suggestion: &str,
    width: u16,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let text_w = widgets::box_text_width(width) as usize;
    let new_lines: Vec<&str> = suggestion.lines().collect();

    let left = vec![
        Span::styled(
            "◆ ",
            Style::default()
                .fg(theme.suggestion)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Suggested change",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ];
    let mut right: Vec<Span<'static>> = Vec::new();
    if anchor.is_some() {
        right.push(Span::styled("-1", Style::default().fg(theme.diff_removed)));
    }
    if !new_lines.is_empty() {
        if !right.is_empty() {
            right.push(Span::raw(" "));
        }
        right.push(Span::styled(
            format!("+{}", new_lines.len()),
            Style::default().fg(theme.diff_added),
        ));
    }
    let header = Line::from(widgets::justify_between(left, right, text_w));

    let start = anchor.map(|(n, _)| n);
    let num_width = start
        .map_or(0, |n| (n + new_lines.len().saturating_sub(1)).to_string().len());
    let mut rows: Vec<Line<'static>> = Vec::new();
    if let Some((n, old)) = anchor {
        rows.push(widgets::numbered_diff_row(
            Some(n as u32),
            num_width,
            "-",
            old,
            theme.diff_removed,
            Some(theme.diff_removed_bg),
            theme.muted,
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
            Some(theme.diff_added_bg),
            theme.fg,
            text_w,
        ));
    }

    widgets::boxed(header, rows, width, theme.suggestion)
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
        let mut new_no = hunk.new_start;
        let mut old_no = hunk.old_start;
        for dl in &hunk.lines {
            rows.push(Row { dl, new_no, old_no, hunk: hunk_idx });
            match dl {
                DiffLine::Added(_) => new_no += 1,
                DiffLine::Removed(_) => old_no += 1,
                DiffLine::Context(_) => {
                    new_no += 1;
                    old_no += 1;
                }
            }
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
    let anchor_text = match rows[anchor].dl {
        DiffLine::Added(c) | DiffLine::Removed(c) | DiffLine::Context(c) => c.clone(),
    };

    // Context must not bleed in from the previous hunk (non-adjacent lines).
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
                Some(theme.diff_added_bg),
                theme.fg,
                row_w,
            ),
            DiffLine::Removed(c) => widgets::numbered_diff_row(
                None,
                num_width,
                "-",
                c,
                theme.diff_removed,
                Some(theme.diff_removed_bg),
                theme.muted,
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
