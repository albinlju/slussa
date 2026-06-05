use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{
    app::state::{AppState, LoadState},
    domain::{
        diff::{Diff, DiffLine, FileDiff},
        pr::PullRequest,
    },
    tui::{
        pr_detail::file_tree::{TreeRow, build_visible_rows},
        spinner_frame,
    },
};

pub fn render(frame: &mut Frame, pr: &PullRequest, state: &AppState, area: Rect) {
    let diff_state = state.cache.details.get(&pr.id).map(|d| &d.diff);

    match diff_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{} Loading diff...", spinner_frame()))
                .style(Style::default().fg(Color::Yellow));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) if diff.files.is_empty() => {
            let paragraph =
                Paragraph::new("(no diff)").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(area);

            let rows = build_visible_rows(&diff.files, &state.ui.diff.collapsed);
            render_tree(frame, &rows, state.ui.diff.cursor, chunks[0]);
            render_diff_pane(frame, diff, state.ui.diff.focused_file, chunks[1]);
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
                        Span::styled(
                            format!("{} ", marker),
                            Style::default().fg(Color::DarkGray),
                        ),
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

fn render_diff_pane(frame: &mut Frame, diff: &Diff, focused_file: usize, area: Rect) {
    let bounded = focused_file.min(diff.files.len().saturating_sub(1));
    let file = match diff.files.get(bounded) {
        Some(f) => f,
        None => return,
    };

    let lines = file_to_lines(file);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn file_to_lines(file: &FileDiff) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::styled(
        file.path.clone(),
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));
    for hunk in &file.hunks {
        lines.push(Line::styled(
            format!("@@ -{} +{} @@", hunk.old_start, hunk.new_start),
            Style::default().fg(Color::Cyan),
        ));
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
        }
    }
    lines
}
