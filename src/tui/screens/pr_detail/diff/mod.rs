mod pane;
mod tree;

use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Paragraph,
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
    tui::{theme, widgets},
};

pub fn render(
    frame: &mut Frame,
    diff_state: Option<&LoadState<Diff>>,
    review_threads: &[ReviewThread],
    ui_diff: &mut DiffViewState,
    area: Rect,
) {
    let theme = theme::current();
    let Some(diff) = widgets::loaded_or_placeholder(frame, diff_state, "diff", area) else {
        return;
    };
    if diff.files.is_empty() {
        let paragraph = Paragraph::new("(no diff)").style(Style::default().fg(theme.muted));
        frame.render_widget(paragraph, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let file_stats: Vec<(u32, u32)> = diff.files.iter().map(count_file_stats).collect();

    let comment_counts: Vec<usize> = diff
        .files
        .iter()
        .map(|f| file_comment_count(f, review_threads))
        .collect();

    // Column height minus border (2) and header band (2).
    ui_diff.pane_viewport = chunks[2].height.saturating_sub(4);
    ui_diff.tree_viewport = chunks[0].height.saturating_sub(4);

    let tree_focused = matches!(ui_diff.focus, DiffFocus::Tree);
    let rows = build_visible_rows(&diff.files, &ui_diff.collapsed, &ui_diff.tree_search.query);
    tree::render(
        frame,
        &rows,
        ui_diff.cursor,
        &file_stats,
        &comment_counts,
        tree_focused,
        chunks[0],
    );
    // The highlight + matches apply only once committed (Enter), so the
    // query is withheld while the prompt is still open.
    let pane_query = if ui_diff.pane_search.open {
        ""
    } else {
        ui_diff.pane_search.query.as_str()
    };
    let (pane_items, pane_matches) = pane::render(
        frame,
        diff,
        ui_diff.focused_file,
        &mut ui_diff.pane_scroll,
        ui_diff.pane_cursor,
        &file_stats,
        review_threads,
        !tree_focused,
        pane_query,
        chunks[2],
    );
    ui_diff.pane_items = pane_items;
    ui_diff.pane_matches = pane_matches;
}

/// Comments whose anchored line is actually present in *this* file's diff —
/// so the tree badge matches what the pane renders inline. Mirrors the pane's
/// anchoring: new-side lines (added/context) match `thread.line`, old-side
/// lines (removed) match `thread.old_line`.
fn file_comment_count(file: &FileDiff, threads: &[ReviewThread]) -> usize {
    // Most files have no threads — skip the line-set work for them.
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
        .filter(|t| {
            t.line.is_some_and(|l| new_lines.contains(&l))
                || t.old_line.is_some_and(|o| old_lines.contains(&o))
        })
        .map(|t| t.comments.len())
        .sum()
}

/// Walk a file's hunks and count `+` / `-` lines. Shared between the tree
/// header (sums them) and the pane header (per-file).
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
