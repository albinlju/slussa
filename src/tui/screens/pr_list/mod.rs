use crate::{
    app::{
        action::{Action, ListAction},
        state::{AppState, LoadState, StatusFilter},
    },
    domain::{ci::CiSummary, pr::PullRequest, review::ReviewerState},
    tui::{
        screens::half_page,
        table::{self, Cell, Column, Width},
        theme, widgets,
    },
};
use chrono::Utc;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

// The 2-col gutter reserved for the list's highlight symbol (`▶ `).
const GUTTER: u16 = 2;

const COLS: &[Column] = &[
    Column { title: "#", width: Width::Fixed(7) },
    Column { title: "Status", width: Width::Fixed(10) },
    Column { title: "Author", width: Width::Fixed(18) },
    Column { title: "Title", width: Width::Flex(1) },
    Column { title: "CI", width: Width::Fixed(4) },
    Column { title: "Diff", width: Width::Fixed(12) },
    Column { title: "Comments", width: Width::Fixed(10) },
    Column { title: "Reviews", width: Width::Fixed(9) },
    Column { title: "Age", width: Width::Fixed(8) },
];

enum PrListView<'a> {
    Loaded(Vec<&'a PullRequest>),
    Loading,
    Failed(String),
}

pub(in crate::tui) fn render(frame: &mut Frame, state: &mut AppState, area: ratatui::layout::Rect) {
    let theme = theme::current();

    state.ui.list_viewport = area.height.saturating_sub(4);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let load_view = match &state.cache.prs {
        LoadState::Loaded(_) => PrListView::Loaded(state.filtered_prs()),
        LoadState::Failed(msg) => PrListView::Failed(msg.clone()),
        _ => PrListView::Loading,
    };

    let title_text = match &load_view {
        PrListView::Loaded(prs) => format!(" {} ({}) ", state.ui.list_filter.label(), prs.len()),
        PrListView::Loading => format!(" {} (…) ", state.ui.list_filter.label()),
        PrListView::Failed(_) => format!(" {} (!) ", state.ui.list_filter.label()),
    };
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .title(Line::styled(
            title_text,
            Style::default()
                .fg(theme.orange)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = outer_block.inner(chunks[0]);
    frame.render_widget(outer_block, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let table = table::Table::new(COLS, inner.width.saturating_sub(GUTTER));

    let mut header = table.header();
    header.spans.insert(0, Span::raw(" ".repeat(GUTTER as usize)));
    frame.render_widget(Paragraph::new(header), content_chunks[0]);

    let match_count = match &load_view {
        PrListView::Loaded(prs) => prs.len(),
        _ => 0,
    };
    match load_view {
        PrListView::Loaded(filtered) => {
            let items: Vec<ListItem> = filtered
                .iter()
                .map(|pr| ListItem::new(table.row(&row_cells(pr))))
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(state.ui.list_selected));

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(theme.highlight_bg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");

            frame.render_stateful_widget(list, content_chunks[1], &mut list_state);
        }
        PrListView::Loading => {
            let mut line = widgets::loading("Loading pull requests…");
            line.spans.insert(0, Span::raw("  "));
            frame.render_widget(Paragraph::new(line), content_chunks[1]);
        }
        PrListView::Failed(msg) => {
            let err = Paragraph::new(format!("  Couldn't load pull requests: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(err, content_chunks[1]);
        }
    }

    let footer_line = if state.ui.list_search.open {
        widgets::search_prompt(&state.ui.list_search.query, match_count, chunks[1].width)
    } else {
        widgets::footer(
            chunks[1].width,
            "j/k: navigate  /: search  ^d/^u: page  enter: open  f: filter  q: quit",
        )
    };
    frame.render_widget(Paragraph::new(footer_line), chunks[1]);

    if state.ui.filter_picker_open {
        render_filter_picker(frame, state, area);
    }
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

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

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
    frame.render_stateful_widget(list, inner_chunks[0], &mut list_state);

    let help = Paragraph::new(" j/k: nav  enter: apply  esc: cancel ")
        .style(Style::default().fg(theme.muted));
    frame.render_widget(help, inner_chunks[1]);
}

fn row_cells(pr: &PullRequest) -> Vec<Cell> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);

    let days_old = (Utc::now() - pr.created).num_days();
    let age = if days_old == 0 {
        "today".to_string()
    } else if days_old == 1 {
        "1d".to_string()
    } else {
        format!("{days_old}d")
    };

    let (ci_sym, ci_color) = match pr.ci {
        CiSummary::Success => ("\u{f058}", theme.success), //  check-circle
        CiSummary::Failed => ("\u{f057}", theme.error),    //  times-circle
        CiSummary::Pending => ("\u{f017}", theme.warning), //  clock
        CiSummary::Unknown => ("\u{f042}", theme.muted),   //  adjust (half circle — neutral/not run)
    };

    let comm_text = if pr.comment_count == 0 {
        "—".to_string()
    } else {
        pr.comment_count.to_string()
    };

    let (rev_text, rev_color) = if pr.reviewers.is_empty() {
        ("—".to_string(), theme.muted)
    } else {
        let approved = pr
            .reviewers
            .iter()
            .filter(|r| r.state == ReviewerState::Approved)
            .count();
        let total = pr.reviewers.len();
        let any_blocking = pr
            .reviewers
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
    };

    vec![
        vec![Span::styled(format!("#{}", pr.id), muted)],
        vec![Span::styled(pr.status.label().to_string(), Style::default().fg(theme.status_color(&pr.status)))],
        vec![Span::styled(pr.author.username.clone(), Style::default().fg(theme.info))],
        vec![Span::raw(pr.title.clone())],
        vec![Span::styled(ci_sym, Style::default().fg(ci_color))],
        vec![
            Span::styled(format!("+{}", pr.additions), Style::default().fg(theme.diff_added)),
            Span::raw(" "),
            Span::styled(format!("-{}", pr.deletions), Style::default().fg(theme.diff_removed)),
        ],
        vec![Span::styled(comm_text, muted)],
        vec![Span::styled(rev_text, Style::default().fg(rev_color))],
        vec![Span::styled(age, muted)],
    ]
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
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('d') => Some(Action::List(ListAction::MoveSelection(half))),
            KeyCode::Char('u') => Some(Action::List(ListAction::MoveSelection(-half))),
            _ => None,
        };
    }

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
