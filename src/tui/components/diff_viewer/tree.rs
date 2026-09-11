use crate::{
    domain::diff::Diff,
    tui::{
        components::diff_viewer::{
            DiffViewer,
            file_tree::{TreeRow, build_visible_rows},
        },
        icons, theme, widgets,
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

pub(super) fn render(
    frame: &mut Frame,
    diff: &Diff,
    ui_diff: &mut DiffViewer,
    file_stats: &[(u32, u32)],
    comment_counts: &[usize],
    area: Rect,
) {
    ui_diff.tree_viewport = area.height.saturating_sub(4);
    let rows = build_visible_rows(&diff.files, &ui_diff.collapsed, &ui_diff.tree_search.query);

    let (header_inner, body) = widgets::framed_panel(frame, area);
    let header_width = (header_inner.width as usize).saturating_sub(1);
    frame.render_widget(
        Paragraph::new(tree_header(file_stats, header_width)),
        header_inner,
    );

    let row_width = body.width as usize;
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| ListItem::new(tree_row(row, file_stats, comment_counts, row_width)))
        .collect();

    let bounded_cursor = ui_diff.cursor.min(rows.len().saturating_sub(1));
    let mut list_state = ListState::default();
    list_state.select(Some(bounded_cursor));

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme::current().highlight_bg)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, body, &mut list_state);
}

fn tree_header(file_stats: &[(u32, u32)], width: usize) -> Line<'static> {
    let theme = theme::current();
    let file_count = file_stats.len();
    let (total_adds, total_dels) = file_stats
        .iter()
        .fold((0u32, 0u32), |(a, d), (na, nd)| (a + na, d + nd));

    let left = vec![Span::styled(
        format!("{file_count} files"),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )];
    let right = vec![
        Span::styled(
            format!("+{total_adds}"),
            Style::default().fg(theme.diff_added),
        ),
        Span::raw(" "),
        Span::styled(
            format!("-{total_dels}"),
            Style::default().fg(theme.diff_removed),
        ),
    ];
    Line::from(widgets::justify_between(left, right, width))
}

fn tree_row(
    row: &TreeRow,
    file_stats: &[(u32, u32)],
    comment_counts: &[usize],
    row_width: usize,
) -> Line<'static> {
    let theme = theme::current();
    match row {
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
        } => file_row(
            name,
            *depth,
            *file_index,
            file_stats,
            comment_counts,
            row_width,
        ),
    }
}

fn file_row(
    name: &str,
    depth: usize,
    file_index: usize,
    file_stats: &[(u32, u32)],
    comment_counts: &[usize],
    row_width: usize,
) -> Line<'static> {
    let theme = theme::current();
    let (adds, dels) = file_stats.get(file_index).copied().unwrap_or((0, 0));
    let comments = comment_counts.get(file_index).copied().unwrap_or(0);
    let indent = "  ".repeat(depth + 1);

    let mut right: Vec<Span<'static>> = Vec::new();
    if comments > 0 {
        right.push(Span::styled(
            format!("{} {comments}", icons::COMMENT),
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

    let visible_left = indent.len() + Span::raw(name).width();
    let visible_right: usize = right.iter().map(Span::width).sum();
    let pad = row_width
        .saturating_sub(visible_left + visible_right + 1)
        .max(1);
    let mut spans = vec![
        Span::raw(indent),
        Span::raw(name.to_string()),
        Span::raw(" ".repeat(pad)),
    ];
    spans.extend(right);
    Line::from(spans)
}
