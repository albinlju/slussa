use super::meta;
use crate::{
    domain::{
        comment::{Comment, CommentKey, CommentKind, CommentThread, split_suggestions},
        diff::{Diff, LineRef},
    },
    tui::ui::{
        format, theme,
        widgets::{
            self,
            comment::{
                code::{diff_snippet, suggestion_box},
                fold::{Fold, Folds},
                frame::{bracket, framed, header_line, prefix_gutter, status_rule},
                meta::Reading,
            },
            markdown,
        },
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

/// A thread drawn in the diff, and the rows in it that open or fold a comment.
pub(in crate::tui::ui) struct InlineThread {
    pub lines: Vec<Line<'static>>,
    pub folds: Vec<Fold>,
}

pub(in crate::tui::ui) fn render_inline_thread(
    thread: &CommentThread,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    anchor_text: Option<&str>,
    reading: Reading<'_>,
    expanded: bool,
) -> InlineThread {
    let theme = theme::current();
    let frame = if active { theme.accent } else { theme.divider };

    // A resolved thread collapses to a one-line summary until expanded (`space`).
    if thread.resolved() && !expanded {
        return InlineThread {
            lines: vec![collapse_summary(thread, false, active, width)],
            folds: Vec::new(),
        };
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
    let before = out.len();
    let (lines, _, mut folds) = conversation(thread, pos, width, now, reading, frame, None, false);
    out.extend(lines);
    for fold in &mut folds {
        fold.row += before;
    }
    InlineThread { lines: out, folds }
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
        if comment.is_ai() {
            spans.push(meta::ai_tag());
        }
    }
    Line::from(widgets::truncate_to_width(spans, width as usize))
}

pub(in crate::tui::ui) fn comment_box(
    comment: &Comment,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    reading: Reading<'_>,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let frame = if active { theme.accent } else { theme.divider };
    let header = header_line(
        meta::meta(comment, reading, now),
        kind_label(comment),
        width,
    );
    let mut out = vec![header];
    out.extend(bracket(
        comment_body(
            comment,
            CommentKind::Conversation,
            None,
            width.saturating_sub(2),
            reading.folds,
        )
        .0,
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

pub(in crate::tui::ui) fn comment_thread_box(
    thread: &CommentThread,
    diff: Option<&Diff>,
    width: u16,
    now: DateTime<Utc>,
    active: bool,
    selected: Option<usize>,
    reading: Reading<'_>,
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
    if first.is_ai() {
        left.push(meta::ai_tag());
    }
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
    let (lines, selected_range, _) =
        conversation(thread, pos, width, now, reading, frame, selected, true);
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

#[expect(clippy::too_many_arguments, reason = "render inputs; see ROADMAP")]
fn conversation(
    thread: &CommentThread,
    anchor: Option<(usize, &str)>,
    width: u16,
    now: DateTime<Utc>,
    reading: Reading<'_>,
    frame: Color,
    selected: Option<usize>,
    skip_first_meta: bool,
) -> (
    Vec<Line<'static>>,
    Option<std::ops::Range<usize>>,
    Vec<Fold>,
) {
    let theme = theme::current();
    let style = Style::default().fg(frame);
    let last = thread.comments.len().saturating_sub(1);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut selected_range = None;
    let mut folds: Vec<Fold> = Vec::new();
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
            meta::meta(comment, reading, now)
        };
        if !split_suggestions(&comment.content).1.is_empty() {
            let label = if suppress {
                Vec::new()
            } else {
                status_label(thread.resolved())
            };
            let (body, fold) = comment_body(
                comment,
                thread.kind(),
                anchor,
                width.saturating_sub(2),
                reading.folds,
            );
            // The frame's top line comes first.
            folds.extend(fold.map(|f| Fold {
                row: out.len() + 1 + f.row,
                ..f
            }));
            out.extend(framed(meta, label, body, width, frame, theme.divider));
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
        let (body, fold) = comment_body(
            comment,
            thread.kind(),
            anchor,
            width.saturating_sub(2),
            reading.folds,
        );
        folds.extend(fold.map(|f| Fold {
            row: out.len() + f.row,
            ..f
        }));
        for line in body {
            out.push(prefix_gutter(line, body_gutter, style));
        }
        if selected == Some(i) {
            selected_range = Some(start..out.len());
        }
    }
    (out, selected_range, folds)
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

fn comment_body(
    comment: &Comment,
    kind: CommentKind,
    anchor: Option<(usize, &str)>,
    text_w: u16,
    folds: Folds<'_>,
) -> (Vec<Line<'static>>, Option<Fold>) {
    let (prose, suggestions) = split_suggestions(&comment.content);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let folded = (!prose.trim().is_empty()).then(|| {
        let key = comment.id.map(|id| CommentKey { id, kind });
        folds.apply(key, paint_fg(markdown::render_no_margin(&prose, text_w)))
    });
    let fold = folded.as_ref().and_then(|folded| folded.fold);
    lines.extend(folded.into_iter().flat_map(|folded| folded.lines));
    for suggestion in &suggestions {
        lines.push(Line::raw(""));
        lines.extend(suggestion_box(anchor, suggestion, text_w));
    }
    if let Some(line) = meta::reactions_line(&comment.reactions) {
        lines.push(Line::raw(""));
        lines.push(line);
    }
    (lines, fold)
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
