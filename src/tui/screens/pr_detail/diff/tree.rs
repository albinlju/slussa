//! Left side of the Diff tab — the file tree with a `N files +A -B`
//! header band on top and the list of dirs/files below.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::tui::{
    screens::pr_detail::file_tree::TreeRow,
    theme,
};

pub(super) fn render(
    frame: &mut Frame,
    rows: &[TreeRow],
    cursor: usize,
    file_stats: &[(u32, u32)],
    area: Rect,
) {
    let theme = theme::current();
    let file_count = file_stats.len();
    let (total_adds, total_dels) = file_stats
        .iter()
        .fold((0u32, 0u32), |(a, d), (na, nd)| (a + na, d + nd));

    let tree_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.divider));
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
    let files_label = format!("{file_count} files");
    let plus = format!("+{total_adds}");
    let minus = format!("-{total_dels}");
    let header_w = header_inner.width as usize;
    let visible_right = plus.chars().count() + 1 + minus.chars().count();
    // Same trailing 1-col gap before the right edge as the file rows below,
    // so the +A -D blocks vertically align.
    let header_pad = header_w
        .saturating_sub(files_label.chars().count() + visible_right + 1)
        .max(1);
    let header_line = Line::from(vec![
        Span::styled(
            files_label,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(header_pad)),
        Span::styled(plus, Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(minus, Style::default().fg(theme.diff_removed)),
    ]);
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
                    let indent = "  ".repeat(*depth + 1);
                    let plus = format!("+{adds}");
                    let minus = format!("-{dels}");
                    let visible_left = indent.chars().count() + name.chars().count();
                    let visible_right = plus.chars().count() + 1 + minus.chars().count();
                    let pad = row_width
                        .saturating_sub(visible_left + visible_right + 1)
                        .max(1);
                    Line::from(vec![
                        Span::raw(indent),
                        Span::raw(name.clone()),
                        Span::raw(" ".repeat(pad)),
                        Span::styled(plus, Style::default().fg(theme.diff_added)),
                        Span::raw(" "),
                        Span::styled(minus, Style::default().fg(theme.diff_removed)),
                    ])
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
