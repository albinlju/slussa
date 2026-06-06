use std::collections::HashMap;

use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{
    app::state::{DiffViewState, LoadState, PrData},
    domain::{
        comment::ReviewThread,
        diff::{Diff, DiffLine, FileDiff},
        pr::PullRequest,
    },
    tui::{
        pr_detail::{
            file_tree::{TreeRow, build_visible_rows},
            render_inline_thread,
        },
        spinner_frame,
    },
};

pub fn render(
    frame: &mut Frame,
    _pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui_diff: &DiffViewState,
    area: Rect,
) {
    let diff_state = pr_data.map(|d| &d.diff);
    let review_threads: &[ReviewThread] = pr_data
        .and_then(|d| match &d.review_threads {
            LoadState::Loaded(t) => Some(t.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);

    match diff_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{} Loading diff...", spinner_frame()))
                .style(Style::default().fg(Color::Yellow));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) if diff.files.is_empty() => {
            let paragraph = Paragraph::new("(no diff)").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(area);

            let rows = build_visible_rows(&diff.files, &ui_diff.collapsed);
            render_tree(frame, &rows, ui_diff.cursor, chunks[0]);
            render_diff_pane(frame, diff, ui_diff.focused_file, review_threads, chunks[1]);
        }
    }
}

fn render_tree(frame: &mut Frame, rows: &[TreeRow], cursor: usize, area: Rect) {
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| {
            let line = match row {
                TreeRow::Dir {
                    name,
                    depth,
                    expanded,
                    ..
                } => {
                    let marker = if *expanded { "▼" } else { "▶" };
                    let indent = "  ".repeat(*depth);
                    Line::from(vec![
                        Span::raw(indent),
                        Span::styled(format!("{} ", marker), Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("{}/", name),
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ])
                }
                TreeRow::File { name, depth, .. } => {
                    let indent = "  ".repeat(*depth + 1);
                    Line::from(vec![Span::raw(indent), Span::raw(name.clone())])
                }
            };
            ListItem::new(line)
        })
        .collect();

    let bounded_cursor = cursor.min(rows.len().saturating_sub(1));
    let mut list_state = ListState::default();
    list_state.select(Some(bounded_cursor));

    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_diff_pane(
    frame: &mut Frame,
    diff: &Diff,
    focused_file: usize,
    threads: &[ReviewThread],
    area: Rect,
) {
    let bounded = focused_file.min(diff.files.len().saturating_sub(1));
    let file = match diff.files.get(bounded) {
        Some(f) => f,
        None => return,
    };

    let lines = file_to_lines(file, threads, area.width);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn file_to_lines(file: &FileDiff, threads: &[ReviewThread], width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::styled(
        file.path.clone(),
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));

    // Build a lookup of (new file line number) -> threads anchored there.
    let mut comments_at: HashMap<usize, Vec<&ReviewThread>> = HashMap::new();
    for thread in threads.iter().filter(|t| t.path == file.path) {
        if let Some(line) = thread.line {
            comments_at.entry(line).or_default().push(thread);
        }
    }

    let now = Utc::now();

    for hunk in &file.hunks {
        lines.push(Line::styled(
            format!("@@ -{} +{} @@", hunk.old_start, hunk.new_start),
            Style::default().fg(Color::Cyan),
        ));

        let mut new_line_num = hunk.new_start;
        for diff_line in &hunk.lines {
            let (prefix, content, color) = match diff_line {
                DiffLine::Added(c) => ("+", c.as_str(), Color::Green),
                DiffLine::Removed(c) => ("-", c.as_str(), Color::Red),
                DiffLine::Context(c) => (" ", c.as_str(), Color::Reset),
            };
            lines.push(Line::styled(
                format!("{}{}", prefix, content),
                Style::default().fg(color),
            ));

            // Removed lines have no new-file line number; comments anchored to a
            // "new" line number only correspond to Added/Context lines.
            let current_line = match diff_line {
                DiffLine::Removed(_) => None,
                _ => Some(new_line_num),
            };
            if let Some(ln) = current_line {
                if let Some(threads_here) = comments_at.get(&ln) {
                    for thread in threads_here {
                        lines.extend(render_inline_thread(thread, width, now));
                    }
                }
            }

            if !matches!(diff_line, DiffLine::Removed(_)) {
                new_line_num += 1;
            }
        }
    }

    lines
}

