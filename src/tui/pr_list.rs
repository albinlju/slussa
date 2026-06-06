use crate::{
    app::state::{AppState, LoadState, StatusFilter},
    domain::review::ReviewerState,
    tui::{Action, spinner_frame},
};
use chrono::Utc;
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    match &state.cache.prs {
        LoadState::NotRequested | LoadState::Loading => {
            let loading = Paragraph::new(format!(
                "\n\n  {}  Loading pull requests...",
                spinner_frame()
            ))
            .block(Block::default().borders(Borders::ALL).title("tuipr"))
            .style(Style::default().fg(Color::Yellow));
            frame.render_widget(loading, chunks[0]);
        }
        LoadState::Loaded(_) => {
            let filtered = state.filtered_prs();
            let outer_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(format!(
                    " {} ({}) ",
                    state.ui.list_filter.label(),
                    filtered.len()
                ));
            let inner = outer_block.inner(chunks[0]);
            frame.render_widget(outer_block, chunks[0]);

            let content_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            let header_style = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);

            let header = Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("  #       ", header_style),
                Span::styled(format!("{:<9}", "Status"), header_style),
                Span::styled(format!("{:<24}", "Author"), header_style),
                Span::styled(format!("{:<47}", "Title"), header_style),
                Span::styled(format!("{:<10}", "Age"), header_style),
                Span::styled("Comments  Reviewers", header_style),
            ]));

            frame.render_widget(header, content_chunks[0]);

            let items: Vec<ListItem> = filtered
                .iter()
                .map(|pr| {
                    let status_color = match pr.status {
                        crate::domain::pr::PrStatus::Draft => Color::DarkGray,
                        crate::domain::pr::PrStatus::Open => Color::Green,
                        crate::domain::pr::PrStatus::Merged => Color::Magenta,
                        crate::domain::pr::PrStatus::Declined => Color::Red,
                    };

                    let days_old = (Utc::now() - pr.created).num_days();
                    let age = if days_old == 0 {
                        "today".to_string()
                    } else if days_old == 1 {
                        "1d".to_string()
                    } else {
                        format!("{}d", days_old)
                    };

                    let reviewer_span = if pr.reviewers.is_empty() {
                        Span::styled("  —", Style::default().fg(Color::DarkGray))
                    } else {
                        let approved = pr
                            .reviewers
                            .iter()
                            .filter(|r| r.state == ReviewerState::Approved)
                            .count();
                        let total = pr.reviewers.len();
                        let color = if approved == total {
                            Color::Green
                        } else {
                            Color::Yellow
                        };
                        Span::styled(
                            format!("  {}/{}", approved, total),
                            Style::default().fg(color),
                        )
                    };

                    let line = Line::from(vec![
                        Span::styled(
                            format!("  #{:<7}", pr.id),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{:<9}", pr.status.label()),
                            Style::default().fg(status_color),
                        ),
                        Span::styled(
                            format!(
                                "{:<24}",
                                pr.author.username.chars().take(22).collect::<String>()
                            ),
                            Style::default().fg(Color::Cyan),
                        ),
                        Span::raw(format!(
                            "{:<47}",
                            pr.title.chars().take(45).collect::<String>()
                        )),
                        Span::styled(format!("{:<10}", age), Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("{:<10}", format!("{} 💬", pr.comment_count)),
                            Style::default().fg(Color::DarkGray),
                        ),
                        reviewer_span,
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(state.ui.list_selected));

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");

            frame.render_stateful_widget(list, content_chunks[1], &mut list_state);
        }
    }

    let status = Paragraph::new("  j/k: navigate  enter: open PR  f: filter  q: quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, chunks[1]);

    if state.ui.filter_picker_open {
        render_filter_picker(frame, state, area);
    }
}

fn render_filter_picker(frame: &mut Frame, state: &AppState, area: Rect) {
    let popup_width = 40u16.min(area.width);
    // 5 filters + 2 border rows + 1 help row = 8
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
        .title(" Filter ")
        .border_style(Style::default().fg(Color::Yellow));
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
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, inner_chunks[0], &mut list_state);

    let help = Paragraph::new(" j/k: nav  enter: apply  esc: cancel ")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, inner_chunks[1]);
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    if state.ui.filter_picker_open {
        return match key {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Esc | KeyCode::Char('f') => Some(Action::CloseFilterPicker),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::FilterPickerNext),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::FilterPickerPrev),
            KeyCode::Enter => Some(Action::ApplyFilter),
            _ => None,
        };
    }

    match key {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('f') => Some(Action::OpenFilterPicker),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::NextPr),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::PrevPr),
        KeyCode::Enter => state
            .filtered_prs()
            .get(state.ui.list_selected)
            .map(|p| Action::OpenPr(p.id)),
        _ => None,
    }
}
