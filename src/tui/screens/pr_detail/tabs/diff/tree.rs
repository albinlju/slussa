use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use crate::app::file_tree::TreeRow;
use crate::tui::{theme, widgets};

pub(super) struct TreeView<'a> {
    pub rows: &'a [TreeRow],
    pub cursor: usize,
    pub file_stats: &'a [(u32, u32)],
    pub comment_counts: &'a [usize],
    pub focused: bool,
}

impl TreeView<'_> {
    pub(super) fn render(&self, frame: &mut Frame, area: Rect) {
        let theme = theme::current();
        let file_count = self.file_stats.len();
        let (total_adds, total_dels) = self
            .file_stats
            .iter()
            .fold((0u32, 0u32), |(a, d), (na, nd)| (a + na, d + nd));

        let (header_inner, body) = widgets::framed_panel(frame, area, self.focused);
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

        let row_width = body.width as usize;

        let items: Vec<ListItem> = self
            .rows
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
                        let (adds, dels) =
                            self.file_stats.get(*file_index).copied().unwrap_or((0, 0));
                        let comments = self.comment_counts.get(*file_index).copied().unwrap_or(0);
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

        let bounded_cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        let mut list_state = ListState::default();
        list_state.select(Some(bounded_cursor));

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

        frame.render_stateful_widget(list, body, &mut list_state);
    }
}
