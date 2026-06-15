use crate::{
    app::{
        action::{Action, ListAction},
        state::{AppState, LoadState, SearchState, StatusFilter},
    },
    domain::{
        ci::CiSummary,
        pr::PullRequest,
        review::{Reviewer, ReviewerState},
    },
    tui::{
        icons, layout,
        screens::half_page,
        table::{self, Cell, Column, Width},
        theme, widgets,
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

const GUTTER: u16 = 2;

const COLS: &[Column] = &[
    Column {
        title: "#",
        width: Width::Fixed(7),
    },
    Column {
        title: "Status",
        width: Width::Fixed(10),
    },
    Column {
        title: "Author",
        width: Width::Fixed(18),
    },
    Column {
        title: "Title",
        width: Width::Flex(1),
    },
    Column {
        title: "CI",
        width: Width::Fixed(4),
    },
    Column {
        title: "Diff",
        width: Width::Fixed(12),
    },
    Column {
        title: "Comments",
        width: Width::Fixed(10),
    },
    Column {
        title: "Reviews",
        width: Width::Fixed(9),
    },
    Column {
        title: "Age",
        width: Width::Fixed(8),
    },
];

pub(in crate::tui) fn render(frame: &mut Frame, state: &mut AppState, area: Rect) {
    state.ui.list_viewport = area.height.saturating_sub(4);

    let [body_area, footer_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Min(0), Constraint::Length(1)],
    );

    let filtered = matches!(state.cache.prs, LoadState::Loaded(_)).then(|| state.filtered_prs());
    let count_label = match (&filtered, &state.cache.prs) {
        (Some(prs), _) => prs.len().to_string(),
        (_, LoadState::Failed(_)) => "!".to_string(),
        _ => "…".to_string(),
    };

    let container = pr_list_container(state.ui.list_filter, &count_label);
    let inner = container.inner(body_area);
    frame.render_widget(container, body_area);

    let [header_area, rows_area] = layout::split(
        inner,
        Direction::Vertical,
        [Constraint::Length(1), Constraint::Min(0)],
    );
    let table = table::Table::new(COLS, inner.width.saturating_sub(GUTTER));
    render_table_header(frame, &table, header_area);

    if let Some(prs) = &filtered {
        render_table_body(frame, &table, prs, state.ui.list_selected, rows_area);
    } else {
        widgets::loaded_or_placeholder(frame, Some(&state.cache.prs), "pull requests", rows_area);
    }

    let match_count = filtered.as_ref().map_or(0, Vec::len);
    render_footer(frame, &state.ui.list_search, match_count, footer_area);

    if state.ui.filter_picker_open {
        render_filter_picker(frame, state, area);
    }
}

fn pr_list_container(filter: StatusFilter, count_label: &str) -> Block<'static> {
    let theme = theme::current();
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .title(Line::styled(
            format!(" {} ({count_label}) ", filter.label()),
            Style::default()
                .fg(theme.orange)
                .add_modifier(Modifier::BOLD),
        ))
}

fn render_table_header(frame: &mut Frame, table: &table::Table, area: Rect) {
    let mut header = table.header();
    header
        .spans
        .insert(0, Span::raw(" ".repeat(GUTTER as usize)));
    frame.render_widget(Paragraph::new(header), area);
}

fn render_table_body(
    frame: &mut Frame,
    table: &table::Table,
    prs: &[&PullRequest],
    selected: usize,
    area: Rect,
) {
    let theme = theme::current();
    let items: Vec<ListItem> = prs
        .iter()
        .map(|pr| ListItem::new(table.row(&row_cells(pr))))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_footer(frame: &mut Frame, search: &SearchState, match_count: usize, area: Rect) {
    let line = if search.open {
        widgets::search_prompt(&search.query, match_count, area.width)
    } else {
        widgets::footer(
            area.width,
            "j/k: navigate  /: search  ^d/^u: page  enter: open  f: filter  q: quit",
        )
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_filter_picker(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = theme::current();
    let popup_width = 40u16.min(area.width);
    let popup_height = 8u16.min(area.height);
    let popup_area = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height) / 2,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Filter ")
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let [list_area, help_area] = layout::split(
        inner,
        Direction::Vertical,
        [Constraint::Min(0), Constraint::Length(1)],
    );

    let items: Vec<ListItem> = StatusFilter::CYCLE
        .iter()
        .map(|f| ListItem::new(Line::raw(f.label())))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(state.ui.filter_picker_cursor));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, list_area, &mut list_state);

    let help = Paragraph::new(" j/k: nav  enter: apply  esc: cancel ")
        .style(Style::default().fg(theme.muted));
    frame.render_widget(help, help_area);
}

fn row_cells(pr: &PullRequest) -> Vec<Cell> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);

    let age = age_label(pr.created);

    let (ci_sym, ci_color) = match pr.ci {
        CiSummary::Success => (icons::CHECK_CIRCLE, theme.success),
        CiSummary::Failed => (icons::TIMES_CIRCLE, theme.error),
        CiSummary::Pending => (icons::CLOCK, theme.warning),
        // adjust — half circle, neutral/not run
        CiSummary::Unknown => (icons::ADJUST, theme.muted),
    };

    let comm_text = if pr.comment_count == 0 {
        "—".to_string()
    } else {
        pr.comment_count.to_string()
    };

    let (rev_text, rev_color) = review_summary(&pr.reviewers);

    vec![
        vec![Span::styled(format!("#{}", pr.id), muted)],
        vec![Span::styled(
            pr.status.label().to_string(),
            Style::default().fg(theme.status_color(&pr.status)),
        )],
        vec![Span::styled(
            pr.author.username.clone(),
            Style::default().fg(theme.info),
        )],
        vec![Span::raw(pr.title.clone())],
        vec![Span::styled(ci_sym, Style::default().fg(ci_color))],
        vec![
            Span::styled(
                format!("+{}", pr.additions),
                Style::default().fg(theme.diff_added),
            ),
            Span::raw(" "),
            Span::styled(
                format!("-{}", pr.deletions),
                Style::default().fg(theme.diff_removed),
            ),
        ],
        vec![Span::styled(comm_text, muted)],
        vec![Span::styled(rev_text, Style::default().fg(rev_color))],
        vec![Span::styled(age, muted)],
    ]
}

fn age_label(created: DateTime<Utc>) -> String {
    match (Utc::now() - created).num_days() {
        0 => "today".to_string(),
        1 => "1d".to_string(),
        days => format!("{days}d"),
    }
}

fn review_summary(reviewers: &[Reviewer]) -> (String, Color) {
    let theme = theme::current();
    if reviewers.is_empty() {
        return ("—".to_string(), theme.muted);
    }
    let approved = reviewers
        .iter()
        .filter(|r| r.state == ReviewerState::Approved)
        .count();
    let total = reviewers.len();
    let any_blocking = reviewers
        .iter()
        .any(|r| r.state == ReviewerState::ChangesRequested);
    let color = if any_blocking {
        theme.error
    } else if approved == total {
        theme.success
    } else {
        theme.warning
    };
    (format!("{approved}/{total}"), color)
}

pub(in crate::tui) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if state.ui.filter_picker_open {
        return match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Esc | KeyCode::Char('f') => Some(Action::List(ListAction::CloseFilterPicker)),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::List(ListAction::FilterPickerNext)),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::List(ListAction::FilterPickerPrev)),
            KeyCode::Enter => Some(Action::List(ListAction::ApplyFilter)),
            _ => None,
        };
    }

    let half = half_page(state.ui.list_viewport);
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('f') => Some(Action::List(ListAction::OpenFilterPicker)),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::List(ListAction::MoveSelection(1))),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::List(ListAction::MoveSelection(-1))),
        KeyCode::PageDown => Some(Action::List(ListAction::MoveSelection(half))),
        KeyCode::PageUp => Some(Action::List(ListAction::MoveSelection(-half))),
        KeyCode::Enter => state
            .filtered_prs()
            .get(state.ui.list_selected)
            .map(|p| Action::List(ListAction::OpenPr(p.id))),
        _ => None,
    }
}
