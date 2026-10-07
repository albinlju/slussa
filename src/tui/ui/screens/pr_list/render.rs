//! Drawing the list: the table, its footer and what is in front of it.
use super::{ListContext, ListOverlay, PrListScreen, Sort, StatusFilter, columns::ListColumn};
use crate::{
    domain::{
        attention::attention,
        pr::{AiReview, PullRequest},
    },
    tui::{
        app::store::LoadState,
        ui::{
            component::{Component, saturating_u16},
            components::search_input::SearchInput,
            layout, theme,
            widgets::{self, table},
        },
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

const GUTTER: u16 = 2;

/// What the search field understands, besides words, shown while it is empty.
const SEARCH_HINT: &str = "author:  review:  ci:  merge:";

/// What the AI column's diamonds mean, listed in the help while the column is
/// there.
const AI_LEGEND: &[(&str, &str)] = &[
    ("◆", "AI review: of this version"),
    ("◈", "AI review: of an older version"),
    ("◇", "AI review: none"),
    ("✗", "AI review: asked for changes"),
];
pub(super) const HELP_KEYS: &[(&str, &str)] = &[
    ("j/k / ↑↓", "move up/down"),
    ("enter", "open PR"),
    ("o", "open PR in browser"),
    ("y", "copy PR link"),
    ("/", "search title / author"),
    ("esc", "clear search"),
    ("f", "filter status"),
    ("s", "sort: pick the order"),
    ("L", "load more PRs"),
    ("^d/^u", "half-page"),
    ("F", "refresh"),
    ("? / esc", "close help"),
    ("q", "quit"),
];

pub(super) fn render(
    screen: &mut PrListScreen,
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &ListContext<'_>,
) {
    screen.viewport = area.height.saturating_sub(4);

    let [body_area, footer_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Min(0), Constraint::Length(1)],
    );

    let filtered = matches!(ctx.prs, LoadState::Loaded(_)).then(|| screen.filtered_prs(ctx));
    let count_label = match (&filtered, ctx.prs) {
        (Some(prs), _) if ctx.loading_more => format!("{}, loading more...", prs.len()),
        (Some(prs), _) => prs.len().to_string(),
        (None, LoadState::Failed(_)) => "!".to_string(),
        (None, LoadState::NotRequested | LoadState::Loading | LoadState::Loaded(_)) => {
            "…".to_string()
        }
    };

    let container = pr_list_container(screen.filter, &count_label, ctx.more);
    let inner = container.inner(body_area);
    frame.render_widget(container, body_area);

    let [header_area, rows_area] = layout::split(
        inner,
        Direction::Vertical,
        [Constraint::Length(1), Constraint::Min(0)],
    );
    let width = inner.width.saturating_sub(GUTTER);
    // The attention column takes its space only while some row has a reason.
    let any_reason = filtered
        .as_ref()
        .is_some_and(|prs| prs.iter().any(|pr| attention(pr, ctx.viewer).is_some()));
    let any_conflict = filtered
        .as_ref()
        .is_some_and(|prs| prs.iter().any(|pr| pr.status.has_conflicts()));
    // So does the AI column: only while some PR has been reviewed by an agent, so
    // a repository without one has no column of hollow diamonds.
    let any_ai = filtered
        .as_ref()
        .is_some_and(|prs| prs.iter().any(|pr| pr.ai_review != AiReview::None));
    let columns: Vec<ListColumn> = ListColumn::visible(width, any_conflict)
        .iter()
        .copied()
        .filter(|&column| column != ListColumn::Attention || any_reason)
        .filter(|&column| column != ListColumn::Ai || any_ai)
        // The status says which view this is, except where the views are mixed, and
        // a conflict is worth a column wherever there is one.
        .filter(|&column| {
            column != ListColumn::Status || screen.filter == StatusFilter::All || any_conflict
        })
        .collect();
    let definitions: Vec<_> = columns.iter().map(|column| column.spec()).collect();
    let table = table::Table::new(&definitions, width);
    render_table_header(frame, &table, header_area);

    if let Some(prs) = &filtered {
        if prs.is_empty() {
            if ctx.view_loading {
                let text = format!("Loading {} PRs...", screen.filter.label().to_lowercase());
                frame.render_widget(Paragraph::new(widgets::loading(&text)), rows_area);
            } else {
                let message = if screen.search.query.is_empty() {
                    "No PRs in this view. f changes filter; F refreshes."
                } else {
                    "No matching PRs. Esc clears search; f changes filter."
                };
                frame.render_widget(widgets::empty_state(message), rows_area);
            }
        } else {
            screen.list_state.select(Some(screen.selected));
            render_table_body(
                frame,
                &table,
                prs,
                &mut screen.list_state,
                rows_area,
                &columns,
                ctx,
            );
        }
    } else {
        widgets::loaded_or_placeholder(frame, Some(ctx.prs), "pull requests", rows_area);
    }

    let match_count = filtered.as_ref().map_or(0, Vec::len);
    let can_load_older = PrListScreen::can_load_older(ctx);
    render_footer(
        frame,
        &screen.search,
        match_count,
        ctx.refreshing,
        can_load_older,
        footer_area,
    );

    let has_link = filtered
        .as_ref()
        .and_then(|prs| prs.get(screen.selected))
        .is_some_and(|pr| pr.url.is_some());
    match &mut screen.overlay {
        Some(ListOverlay::Help(help)) => {
            let entries: Vec<_> = HELP_KEYS
                .iter()
                .copied()
                .filter(|(key, _)| has_link || !matches!(*key, "o" | "y"))
                .filter(|(key, _)| can_load_older || *key != "L")
                .chain(
                    AI_LEGEND
                        .iter()
                        .copied()
                        .filter(|_| columns.contains(&ListColumn::Ai)),
                )
                .collect();
            help.render(frame, area, &entries.as_slice());
        }
        Some(ListOverlay::FilterPicker { highlighted }) => {
            let rows = StatusFilter::CYCLE.map(StatusFilter::label);
            let at = StatusFilter::CYCLE.iter().position(|f| f == highlighted);
            render_picker(frame, "Filter", &rows, at.unwrap_or(0), area);
        }
        Some(ListOverlay::SortPicker { highlighted }) => {
            let rows = Sort::CYCLE.map(Sort::label);
            let at = Sort::CYCLE.iter().position(|s| s == highlighted);
            render_picker(frame, "Sort", &rows, at.unwrap_or(0), area);
        }
        None => {}
    }
}

fn pr_list_container(filter: StatusFilter, count_label: &str, more_closed: bool) -> Block<'static> {
    let theme = theme::current();
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Line::styled(
            format!(" {} ({count_label}) ", filter.title(more_closed)),
            Style::default()
                .fg(theme.orange)
                .add_modifier(Modifier::BOLD),
        ))
}

fn render_table_header(frame: &mut Frame<'_>, table: &table::Table<'_>, area: Rect) {
    let mut header = table.header();
    header
        .spans
        .insert(0, Span::raw(" ".repeat(GUTTER as usize)));
    frame.render_widget(Paragraph::new(header), area);
}

fn render_table_body(
    frame: &mut Frame<'_>,
    table: &table::Table<'_>,
    prs: &[&PullRequest],
    list_state: &mut ListState,
    area: Rect,
    columns: &[ListColumn],
    ctx: &ListContext<'_>,
) {
    let theme = theme::current();
    let items: Vec<ListItem<'_>> = prs
        .iter()
        .map(|pr| {
            let cells: Vec<_> = columns.iter().map(|column| column.cell(pr, ctx)).collect();
            ListItem::new(table.row(&cells))
        })
        .collect();
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, list_state);
}

fn render_footer(
    frame: &mut Frame<'_>,
    search: &SearchInput,
    match_count: usize,
    refreshing: bool,
    load_older: bool,
    area: Rect,
) {
    let hints = if load_older {
        "enter: open  /: search  f: filter  s: sort  L: more"
    } else {
        "enter: open  /: search  f: filter  s: sort"
    };
    let line = if search.open {
        widgets::search_prompt_with_hint(&search.query, match_count, area.width, SEARCH_HINT)
    } else {
        widgets::footer(
            area.width,
            &widgets::hints_on(hints),
            refreshing.then_some("refreshing"),
        )
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// A picker: `rows` in a dialog, with the one at `highlighted` marked.
fn render_picker(
    frame: &mut Frame<'_>,
    title: &str,
    rows: &[&str],
    highlighted: usize,
    area: Rect,
) {
    let theme = theme::current();
    let list_area = widgets::dialog::frame(
        frame,
        area,
        title,
        (44, saturating_u16(rows.len())),
        &[("j/k", "move"), ("Enter", "select"), ("Esc", "cancel")],
    );
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|row| ListItem::new(Line::raw(*row)))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(highlighted));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, list_area, &mut list_state);
}
