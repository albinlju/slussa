mod pane;
mod tree;

use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};

use crate::{
    app::state::{DiffFocus, DiffViewState, LoadState},
    domain::{
        comment::ReviewThread,
        diff::{Diff, DiffLine, FileDiff},
    },
    tui::{layout, widgets},
};

pub fn render(
    frame: &mut Frame,
    diff_state: Option<&LoadState<Diff>>,
    review_threads: &[ReviewThread],
    ui_diff: &mut DiffViewState,
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
        .map(|f| file_comment_count(f, review_threads))
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
        review_threads,
        pane_focused,
        author,
        pane_area,
    );
}

fn file_comment_count(file: &FileDiff, threads: &[ReviewThread]) -> usize {
    if !threads.iter().any(|t| t.path == file.path) {
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
    threads
        .iter()
        .filter(|t| t.path == file.path)
        .filter(|t| match t.line {
            Some(l) => new_lines.contains(&l),
            None => t.old_line.is_some_and(|o| old_lines.contains(&o)),
        })
        .map(|t| t.comments.len())
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
