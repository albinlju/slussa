use super::{DiffFocus, DiffViewer, pane, tree};
use crate::{
    app::{reviews::PendingComment, store::LoadState},
    domain::{
        comment::CommentThread,
        diff::{Diff, DiffLine, FileDiff},
    },
    tui::{layout, widgets},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};
use std::collections::HashSet;

pub(super) fn render(
    frame: &mut Frame,
    diff_state: Option<&LoadState<Diff>>,
    threads: &[CommentThread],
    pending: &[PendingComment],
    ui_diff: &mut DiffViewer,
    author: &str,
    area: Rect,
) {
    let Some(diff) = widgets::loaded_or_placeholder(frame, diff_state, "diff", area) else {
        return;
    };
    if diff.files.is_empty() {
        frame.render_widget(widgets::empty_state("(no diff)"), area);
        return;
    }

    let [tree_area, _, pane_area] = layout::split(
        area,
        Direction::Horizontal,
        [
            Constraint::Percentage(28),
            Constraint::Length(1),
            Constraint::Min(0),
        ],
    );

    let file_stats: Vec<(u32, u32)> = diff.files.iter().map(count_file_stats).collect();
    let comment_counts: Vec<usize> = diff
        .files
        .iter()
        .map(|f| file_comment_count(f, threads))
        .collect();

    let pane_focused = matches!(ui_diff.focus, DiffFocus::Pane);
    tree::render(
        frame,
        diff,
        ui_diff,
        &file_stats,
        &comment_counts,
        tree_area,
    );
    pane::render(
        frame,
        diff,
        ui_diff,
        &file_stats,
        threads,
        pending,
        pane_focused,
        author,
        pane_area,
    );
}

fn file_comment_count(file: &FileDiff, threads: &[CommentThread]) -> usize {
    // Only anchored (code) threads count toward a file; general discussion doesn't.
    let on_file = || {
        threads
            .iter()
            .filter_map(|t| t.anchor.as_ref().map(|a| (t, a)))
            .filter(|(_, a)| a.path == file.path)
    };
    if on_file().next().is_none() {
        return 0;
    }
    let mut new_lines: HashSet<usize> = HashSet::new();
    let mut old_lines: HashSet<usize> = HashSet::new();
    for hunk in &file.hunks {
        for (line, new_no, old_no) in hunk.numbered_lines() {
            match line {
                DiffLine::Added(_) => {
                    new_lines.insert(new_no);
                }
                DiffLine::Removed(_) => {
                    old_lines.insert(old_no);
                }
                DiffLine::Context(_) => {
                    new_lines.insert(new_no);
                    old_lines.insert(old_no);
                }
            }
        }
    }
    on_file()
        .filter(|(_, a)| match a.line {
            Some(l) => new_lines.contains(&l),
            None => a.old_line.is_some_and(|o| old_lines.contains(&o)),
        })
        .map(|(t, _)| t.comments.len())
        .sum()
}

fn count_file_stats(file: &FileDiff) -> (u32, u32) {
    let mut adds = 0u32;
    let mut dels = 0u32;
    for hunk in &file.hunks {
        for line in &hunk.lines {
            match line {
                DiffLine::Added(_) => adds += 1,
                DiffLine::Removed(_) => dels += 1,
                DiffLine::Context(_) => {}
            }
        }
    }
    (adds, dels)
}
