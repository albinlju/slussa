use crate::{
    domain::diff::Diff,
    tui::ui::{
        components::diff_viewer::{
            DiffViewer,
            file_tree::{FileComments, TreeRow, build_visible_rows},
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
    frame: &mut Frame<'_>,
    diff: &Diff,
    ui_diff: &mut DiffViewer,
    file_stats: &[(u32, u32)],
    comment_counts: &[FileComments],
    area: Rect,
) {
    ui_diff.tree_viewport = area.height.saturating_sub(4);
    let rows = build_visible_rows(&diff.files, &ui_diff.collapsed, &ui_diff.tree_search.query);

    let (header_inner, body) = widgets::framed_panel(
        frame,
        area,
        "Files",
        ui_diff.focus == super::DiffFocus::Tree,
    );
    let header_width = (header_inner.width as usize).saturating_sub(1);
    frame.render_widget(
        Paragraph::new(tree_header(file_stats, header_width)),
        header_inner,
    );

    ui_diff.tree_viewport = body.height;
    if rows.is_empty() {
        frame.render_widget(
            widgets::empty_state("No matching files. Esc clears search."),
            body,
        );
        return;
    }
    let row_width = body.width as usize;
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|row| ListItem::new(tree_row(row, file_stats, comment_counts, row_width)))
        .collect();

    let bounded_cursor = ui_diff.cursor.min(rows.len().saturating_sub(1));
    let mut list_state = ListState::default();
    list_state.select(Some(bounded_cursor));

    let list = List::new(items).highlight_style(if ui_diff.focus == super::DiffFocus::Tree {
        Style::default()
            .bg(theme::current().highlight_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::current().muted)
    });
    frame.render_stateful_widget(list, body, &mut list_state);
}

fn tree_header(file_stats: &[(u32, u32)], width: usize) -> Line<'static> {
    let theme = theme::current();
    let file_count = file_stats.len();
    let (total_adds, total_dels) = file_stats
        .iter()
        .fold((0u32, 0u32), |(a, d), (na, nd)| (a + na, d + nd));

    let left = vec![Span::styled(
        format!(
            " {file_count} {}",
            if file_count == 1 { "file" } else { "files" }
        ),
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
    comment_counts: &[FileComments],
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
    comment_counts: &[FileComments],
    row_width: usize,
) -> Line<'static> {
    let theme = theme::current();
    let (adds, dels) = file_stats.get(file_index).copied().unwrap_or((0, 0));
    let comments = comment_counts.get(file_index).copied().unwrap_or_default();
    let indent = "  ".repeat(depth + 1);
    let visible_left = indent.len() + Span::raw(name).width();

    let stats = vec![
        Span::styled(format!("+{adds}"), Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(format!("-{dels}"), Style::default().fg(theme.diff_removed)),
    ];
    let people = (comments.people > 0).then(|| {
        Span::styled(
            format!("{} {}", icons::COMMENT, comments.people),
            Style::default().fg(theme.info),
        )
    });
    let ai = (comments.ai > 0).then(|| {
        Span::styled(
            format!("{} {}", icons::AI, comments.ai),
            Style::default()
                .fg(theme.decorative)
                .add_modifier(Modifier::BOLD),
        )
    });
    // The numbers beside the name are what the narrow list can lose, so the
    // people's count goes first and the AI count next; the line stats stay.
    let width_of = |spans: &[Span<'static>]| -> usize { spans.iter().map(Span::width).sum() };
    let fits = |right: &[Span<'static>]| visible_left + width_of(right) < row_width;
    let with = |badges: Vec<Span<'static>>| -> Vec<Span<'static>> {
        let mut right: Vec<Span<'static>> = badges
            .into_iter()
            .flat_map(|badge| [badge, Span::raw("  ")])
            .collect();
        right.extend(stats.clone());
        right
    };
    let right = [
        with(people.iter().chain(&ai).cloned().collect()),
        with(ai.iter().cloned().collect()),
        with(Vec::new()),
    ]
    .into_iter()
    .find(|right| fits(right))
    .unwrap_or_else(|| with(Vec::new()));

    let visible_right = width_of(&right);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(width: usize) -> String {
        let counts = [FileComments { people: 3, ai: 2 }];
        file_row("worker.rs", 0, 0, &[(12, 4)], &counts, width).to_string()
    }

    #[test]
    fn a_narrow_list_drops_the_peoples_count_first_then_the_agents_and_keeps_the_stats() {
        let all = row(40);
        assert!(all.contains("• 3") && all.contains("◆ 2") && all.ends_with("+12 -4"));
        let without_people = row(26);
        assert!(!without_people.contains('•'), "{without_people}");
        assert!(without_people.contains("◆ 2") && without_people.ends_with("+12 -4"));
        let stats_only = row(20);
        assert!(
            !stats_only.contains('◆') && !stats_only.contains('•'),
            "{stats_only}"
        );
        assert!(stats_only.ends_with("+12 -4"));
    }
}
