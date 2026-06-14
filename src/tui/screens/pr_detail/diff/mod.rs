mod pane;
mod tree;

use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};

use crate::{
    app::{
        file_tree::build_visible_rows,
        state::{DiffFocus, DiffViewState, LoadState},
    },
    domain::{
        comment::ReviewThread,
        diff::{Diff, DiffLine, FileDiff},
    },
    tui::{
        layout,
        screens::pr_detail::diff::{pane::PaneView, tree::TreeView},
        widgets,
    },
};

pub fn render(
    frame: &mut Frame,
    diff_state: Option<&LoadState<Diff>>,
    review_threads: &[ReviewThread],
    ui_diff: &mut DiffViewState,
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
        [Constraint::Percentage(28), Constraint::Length(1), Constraint::Min(0)],
    );

    let file_stats: Vec<(u32, u32)> = diff.files.iter().map(count_file_stats).collect();

    let comment_counts: Vec<usize> = diff
        .files
        .iter()
        .map(|f| file_comment_count(f, review_threads))
        .collect();

    ui_diff.pane_viewport = pane_area.height.saturating_sub(4);
    ui_diff.tree_viewport = tree_area.height.saturating_sub(4);

    let tree_focused = matches!(ui_diff.focus, DiffFocus::Tree);
    let rows = build_visible_rows(&diff.files, &ui_diff.collapsed, &ui_diff.tree_search.query);
    TreeView {
        rows: &rows,
        cursor: ui_diff.cursor,
        file_stats: &file_stats,
        comment_counts: &comment_counts,
        focused: tree_focused,
    }
    .render(frame, tree_area);

    let pane_query = if ui_diff.pane_search.open {
        ""
    } else {
        ui_diff.pane_search.query.as_str()
    };
    let (pane_item_count, pane_matches) = PaneView {
        diff,
        focused_file: ui_diff.focused_file,
        pane_cursor: ui_diff.pane_cursor,
        file_stats: &file_stats,
        threads: review_threads,
        focused: !tree_focused,
        query: pane_query,
    }
    .render(frame, &mut ui_diff.pane_scroll, pane_area);
    ui_diff.pane_item_count = pane_item_count;
    ui_diff.pane_matches = pane_matches;
}

fn file_comment_count(file: &FileDiff, threads: &[ReviewThread]) -> usize {
    if !threads.iter().any(|t| t.path == file.path) {
        return 0;
    }
    let mut new_lines: HashSet<usize> = HashSet::new();
    let mut old_lines: HashSet<usize> = HashSet::new();
    for hunk in &file.hunks {
        let mut new_no = hunk.new_start;
        let mut old_no = hunk.old_start;
        for line in &hunk.lines {
            match line {
                DiffLine::Added(_) => {
                    new_lines.insert(new_no);
                    new_no += 1;
                }
                DiffLine::Removed(_) => {
                    old_lines.insert(old_no);
                    old_no += 1;
                }
                DiffLine::Context(_) => {
                    new_lines.insert(new_no);
                    new_no += 1;
                    old_no += 1;
                }
            }
        }
    }
    threads
        .iter()
        .filter(|t| t.path == file.path)
        // Bucket each thread to one side exactly as the pane does (new-side
        // `line` wins; otherwise old-side `old_line`) so the badge count can't
        // diverge from the threads actually rendered.
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
