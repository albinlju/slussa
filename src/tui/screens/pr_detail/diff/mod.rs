//! Diff tab — left side is the file tree (`tree.rs`), right side is the
//! diff pane (`pane.rs`). This module is the entry point: it picks apart
//! the `LoadState`, computes per-file `+/-` stats once, and routes to the
//! two panel renderers.

mod pane;
mod tree;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Paragraph,
};

use crate::{
    app::state::{DiffFocus, DiffViewState, LoadState, PrData},
    domain::{
        comment::ReviewThread,
        diff::{DiffLine, FileDiff},
    },
    tui::{screens::pr_detail::file_tree::build_visible_rows, theme, widgets},
};

pub fn render(
    frame: &mut Frame,
    pr_data: Option<&PrData>,
    ui_diff: &mut DiffViewState,
    area: Rect,
) {
    let diff_state = pr_data.map(|d| &d.diff);
    let review_threads: &[ReviewThread] = pr_data
        .and_then(|d| match &d.activity {
            LoadState::Loaded(b) => Some(b.threads.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);

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

            // FileDiff doesn't carry stats so we tally them once here and
            // pass the slice down to both panel renderers.
            let file_stats: Vec<(u32, u32)> = diff.files.iter().map(count_file_stats).collect();

            // Body heights = each column minus its border (2) and header band
            // (2); see `pane::render` / `tree::render`. Stored for half-page
            // scrolling and cursor jumps.
            ui_diff.pane_viewport = chunks[2].height.saturating_sub(4);
            ui_diff.tree_viewport = chunks[0].height.saturating_sub(4);

            let tree_focused = matches!(ui_diff.focus, DiffFocus::Tree);
            let rows = build_visible_rows(&diff.files, &ui_diff.collapsed);
            tree::render(
                frame,
                &rows,
                ui_diff.cursor,
                &file_stats,
                tree_focused,
                chunks[0],
            );
            pane::render(
                frame,
                diff,
                ui_diff.focused_file,
                &mut ui_diff.pane_scroll,
                &file_stats,
                review_threads,
                !tree_focused,
                chunks[2],
            );
        }
    }
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
