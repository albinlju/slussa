use crate::{
    app::state::{AppState, LoadState, StatusFilter},
    domain::{ci::CiState, review::ReviewerState},
    tui::{Action, spinner_frame, theme},
};
use chrono::Utc;
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let theme = theme::current();
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
            .style(Style::default().fg(theme.warning));
            frame.render_widget(loading, chunks[0]);
        }
        LoadState::Loaded(_) => {
            let filtered = state.filtered_prs();
            let outer_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .border_type(BorderType::Rounded)
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

            let header_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);

            // The List below shifts every row right by 2 cols to make room for
            // the highlight_symbol ("▶ "). We add the same `  ` prefix to the
            // header so the columns line up.
            let header = Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{:<7}", "#"), header_style),
                Span::styled(format!("{:<10}", "Status"), header_style),
                Span::styled(format!("{:<18}", "Author"), header_style),
                Span::styled(format!("{:<40}", "Title"), header_style),
                Span::styled(format!("{:<3}", "CI"), header_style),
                Span::styled(format!("{:<12}", "Diff"), header_style),
                Span::styled(format!("{:<6}", "Comm"), header_style),
                Span::styled(format!("{:<7}", "Rev"), header_style),
                Span::styled(format!("{:<8}", "Age"), header_style),
            ]));

            frame.render_widget(header, content_chunks[0]);

            let items: Vec<ListItem> = filtered.iter().map(|pr| row_for_pr(pr)).collect();

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
    }

    let status = Paragraph::new("  j/k: navigate  enter: open PR  f: filter  q: quit")
        .style(Style::default().fg(theme.muted));
    frame.render_widget(status, chunks[1]);

    if state.ui.filter_picker_open {
        render_filter_picker(frame, state, area);
    }
}

fn render_filter_picker(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = theme::current();
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

fn row_for_pr(pr: &crate::domain::pr::PullRequest) -> ListItem<'static> {
    let theme = theme::current();
    let status_color = match pr.status {
        crate::domain::pr::PrStatus::Draft => theme.status_draft,
        crate::domain::pr::PrStatus::Open => theme.status_open,
        crate::domain::pr::PrStatus::Merged => theme.status_merged,
        crate::domain::pr::PrStatus::Declined => theme.status_declined,
    };

    let days_old = (Utc::now() - pr.created).num_days();
    let age = if days_old == 0 {
        "today".to_string()
    } else if days_old == 1 {
        "1d".to_string()
    } else {
        format!("{}d", days_old)
    };

    let (ci_sym, ci_color) = match pr.ci.state {
        CiState::Success => ("✓", theme.success),
        CiState::Failed => ("✗", theme.error),
        CiState::Pending => ("●", theme.warning),
        CiState::Unknown => ("—", theme.muted),
    };

    // Diff column is 12 wide total. We render `+N` and `-N` as separate
    // colored spans, so we compute the visible width manually to know how
    // much trailing padding to add.
    let plus = format!("+{}", pr.additions);
    let minus = format!("-{}", pr.deletions);
    let diff_visible = plus.chars().count() + 1 + minus.chars().count();
    let diff_pad = 12usize.saturating_sub(diff_visible);

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
        (format!("{}/{}", approved, total), color)
    };

    let title: String = pr.title.chars().take(38).collect();
    let author: String = pr.author.username.chars().take(16).collect();

    let line = Line::from(vec![
        Span::styled(format!("#{:<6}", pr.id), Style::default().fg(theme.muted)),
        Span::styled(
            format!("{:<10}", pr.status.label()),
            Style::default().fg(status_color),
        ),
        Span::styled(format!("{:<18}", author), Style::default().fg(theme.info)),
        Span::raw(format!("{:<40}", title)),
        Span::styled(format!("{:<3}", ci_sym), Style::default().fg(ci_color)),
        Span::styled(plus, Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(minus, Style::default().fg(theme.diff_removed)),
        Span::raw(" ".repeat(diff_pad)),
        Span::styled(
            format!("{:<6}", comm_text),
            Style::default().fg(theme.muted),
        ),
        Span::styled(format!("{:<7}", rev_text), Style::default().fg(rev_color)),
        Span::styled(format!("{:<8}", age), Style::default().fg(theme.muted)),
    ]);
    ListItem::new(line)
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
