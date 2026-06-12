//! The Diff tab's file tree, with a `N files +A -B` header band.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::file_tree::TreeRow;
use crate::tui::{theme, widgets};

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame,
    rows: &[TreeRow],
    cursor: usize,
    file_stats: &[(u32, u32)],
    comment_counts: &[usize],
    focused: bool,
    area: Rect,
) {
    let theme = theme::current();
    let file_count = file_stats.len();
    let (total_adds, total_dels) = file_stats
        .iter()
        .fold((0u32, 0u32), |(a, d), (na, nd)| (a + na, d + nd));

    let border_color = if focused { theme.accent } else { theme.divider };
    let tree_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let tree_inner = tree_block.inner(area);
    frame.render_widget(tree_block, area);

    let tree_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header row + bottom divider
            Constraint::Min(0),    // file list
        ])
        .split(tree_inner);

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_block.inner(tree_chunks[0]);
    frame.render_widget(header_block, tree_chunks[0]);
    // The `- 1` keeps a trailing 1-col gap before the right edge so the
    // +A -D blocks vertically align with the file rows below.
    let row_width = (header_inner.width as usize).saturating_sub(1);
    let left = vec![Span::styled(
        format!("{file_count} files"),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )];
    let right = vec![
        Span::styled(format!("+{total_adds}"), Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(format!("-{total_dels}"), Style::default().fg(theme.diff_removed)),
    ];
    let header_line = Line::from(widgets::justify_between(left, right, row_width));
    frame.render_widget(Paragraph::new(header_line), header_inner);

    let row_width = tree_chunks[1].width as usize;

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
                        Span::styled(format!("{marker} "), Style::default().fg(theme.muted)),
                        Span::styled(
                            format!("{name}/"),
                            Style::default().fg(theme.link).add_modifier(Modifier::BOLD),
                        ),
                    ])
                }
                TreeRow::File {
                    name,
                    depth,
                    file_index,
                } => {
                    let (adds, dels) = file_stats.get(*file_index).copied().unwrap_or((0, 0));
                    let comments = comment_counts.get(*file_index).copied().unwrap_or(0);
                    let indent = "  ".repeat(*depth + 1);

                    let mut right: Vec<Span<'static>> = Vec::new();
                    if comments > 0 {
                        right.push(Span::styled(
                            format!("\u{f075} {comments}"), //  comment
                            Style::default().fg(theme.info),
                        ));
                        right.push(Span::raw("  "));
                    }
                    right.push(Span::styled(
                        format!("+{adds}"),
                        Style::default().fg(theme.diff_added),
                    ));
                    right.push(Span::raw(" "));
                    right.push(Span::styled(
                        format!("-{dels}"),
                        Style::default().fg(theme.diff_removed),
                    ));

                    let visible_left = indent.len() + Span::raw(name.as_str()).width();
                    let visible_right: usize = right.iter().map(Span::width).sum();
                    let pad = row_width
                        .saturating_sub(visible_left + visible_right + 1)
                        .max(1);
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::raw(name.clone()),
                        Span::raw(" ".repeat(pad)),
                    ];
                    spans.extend(right);
                    Line::from(spans)
                }
            };
            ListItem::new(line)
        })
        .collect();

    let bounded_cursor = cursor.min(rows.len().saturating_sub(1));
    let mut list_state = ListState::default();
    list_state.select(Some(bounded_cursor));

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.highlight_bg)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(list, tree_chunks[1], &mut list_state);
}
