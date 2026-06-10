//! Diff tab — file tree (`tree.rs`) on the left, diff pane (`pane.rs`) on
//! the right.

mod pane;
mod tree;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Paragraph,
};

use crate::{
    app::state::{DiffFocus, DiffViewState, LoadState},
    domain::{
        comment::ReviewThread,
        diff::{Diff, DiffLine, FileDiff},
    },
    tui::{screens::pr_detail::file_tree::build_visible_rows, theme, widgets},
};

/// Renders the full PR diff (Diff tab) or a single commit's diff (Commits
/// drill-in), depending on the `LoadState` passed in.
pub fn render(
    frame: &mut Frame,
    diff_state: Option<&LoadState<Diff>>,
    review_threads: &[ReviewThread],
    ui_diff: &mut DiffViewState,
    area: Rect,
) {
    let theme = theme::current();
    match diff_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{} Loading diff...", widgets::spinner_frame()))
                .style(Style::default().fg(theme.warning));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Failed(msg)) => {
            let paragraph = Paragraph::new(format!("Couldn't load diff: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) if diff.files.is_empty() => {
            let paragraph = Paragraph::new("(no diff)").style(Style::default().fg(theme.muted));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(28),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(area);

            let file_stats: Vec<(u32, u32)> = diff.files.iter().map(count_file_stats).collect();

            // Per-file comment counts for the tree's badge.
            let comment_counts: Vec<usize> = diff
                .files
                .iter()
                .map(|f| {
                    review_threads
                        .iter()
                        .filter(|t| t.path == f.path)
                        .map(|t| t.comments.len())
                        .sum()
                })
                .collect();

            // Column height minus border (2) and header band (2).
            ui_diff.pane_viewport = chunks[2].height.saturating_sub(4);
            ui_diff.tree_viewport = chunks[0].height.saturating_sub(4);

            let tree_focused = matches!(ui_diff.focus, DiffFocus::Tree);
            let rows = build_visible_rows(&diff.files, &ui_diff.collapsed);
            tree::render(
                frame,
                &rows,
                ui_diff.cursor,
                &file_stats,
                &comment_counts,
                tree_focused,
                chunks[0],
            );
            ui_diff.pane_items = pane::render(
                frame,
                diff,
                ui_diff.focused_file,
                &mut ui_diff.pane_scroll,
                ui_diff.pane_cursor,
                &file_stats,
                review_threads,
                !tree_focused,
                chunks[2],
            );
        }
    }
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
