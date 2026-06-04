use crate::{
    app::state::{AppState, Mode},
    domain::review::ReviewerState,
    tui::Outcome,
};
use chrono::Utc;
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    if state.loading {
        let loading = Paragraph::new("\n\n  ⠋ Loading pull requests...")
            .block(Block::default().borders(Borders::ALL).title("tuipr"))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(loading, chunks[0]);
    } else {
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));
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

        let prs: Vec<ListItem> = state
            .prs
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
        list_state.select(Some(state.selected_pr));

        let list = List::new(prs)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, content_chunks[1], &mut list_state);
    }

    let status = Paragraph::new("  j/k: navigate  enter: open PR  q: quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, chunks[1]);
}

pub fn handle_key(state: &mut AppState, key: KeyCode) -> Outcome {
    match key {
        KeyCode::Char('q') => Outcome::Quit,
        KeyCode::Down | KeyCode::Char('j') => {
            let last = state.prs.len().saturating_sub(1);
            state.selected_pr = (state.selected_pr + 1).min(last);
            Outcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.selected_pr = state.selected_pr.saturating_sub(1);
            Outcome::Continue
        }
        KeyCode::Enter => {
            if !state.prs.is_empty() {
                state.mode = Mode::PrDetail;
            }
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}
