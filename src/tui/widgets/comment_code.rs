//! The code a comment is about: the lines a suggestion would replace and the
//! diff around a thread's anchor.

use super::comment_frame::framed;
use crate::{
    domain::diff::{Diff, DiffLine, LineRef},
    tui::{theme, widgets},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub(super) fn suggestion_box(
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

pub(super) fn diff_snippet(
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
