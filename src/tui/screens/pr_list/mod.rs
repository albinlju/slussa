use crate::{
    app::{
        action::{Action, ListAction},
        state::{AppState, LoadState, StatusFilter},
    },
    domain::{ci::CiState, pr::PullRequest, review::ReviewerState},
    tui::{format, screens::half_page, theme, widgets},
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

/// Fixed widths for every column except Title, which flexes to absorb the
/// remaining width.
struct ColWidths {
    id: usize,
    status: usize,
    author: usize,
    title: usize,
    ci: usize,
    diff: usize,
    comments: usize,
    reviews: usize,
    age: usize,
}

impl ColWidths {
    fn for_inner(inner_width: u16) -> Self {
        let id = 7;
        let status = 10;
        let author = 18;
        let ci = 4;
        let diff = 12;
        let comments = 10;
        let reviews = 9;
        let age = 8;
        let fixed = 2 + id + status + author + ci + diff + comments + reviews + age;
        let title = (inner_width as usize).saturating_sub(fixed).max(20);
        Self {
            id,
            status,
            author,
            title,
            ci,
            diff,
            comments,
            reviews,
            age,
        }
    }
}

enum PrListView<'a> {
    Loaded(Vec<&'a PullRequest>),
    Loading,
    Failed(String),
}

pub(in crate::tui) fn render(frame: &mut Frame, state: &mut AppState, area: ratatui::layout::Rect) {
    let theme = theme::current();

    // area minus status bar (1), block border (2), column header (1).
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

    let widths = ColWidths::for_inner(inner.width);
    let header_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);

    let header = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{:<w$}", "#", w = widths.id), header_style),
        Span::styled(format!("{:<w$}", "Status", w = widths.status), header_style),
        Span::styled(format!("{:<w$}", "Author", w = widths.author), header_style),
        Span::styled(format!("{:<w$}", "Title", w = widths.title), header_style),
        Span::styled(format!("{:<w$}", "CI", w = widths.ci), header_style),
        Span::styled(format!("{:<w$}", "Diff", w = widths.diff), header_style),
        Span::styled(
            format!("{:<w$}", "Comments", w = widths.comments),
            header_style,
        ),
        Span::styled(
            format!("{:<w$}", "Reviews", w = widths.reviews),
            header_style,
        ),
        Span::styled(format!("{:<w$}", "Age", w = widths.age), header_style),
    ]));
    frame.render_widget(header, content_chunks[0]);

    let match_count = match &load_view {
        PrListView::Loaded(prs) => prs.len(),
        _ => 0,
    };
    match load_view {
        PrListView::Loaded(filtered) => {
            let items: Vec<ListItem> = filtered.iter().map(|pr| row_for_pr(pr, &widths)).collect();

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
            // Indented to line up with the list rows' highlight-symbol column.
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

fn row_for_pr(pr: &PullRequest, widths: &ColWidths) -> ListItem<'static> {
    let theme = theme::current();
    let status_color = theme.status_color(&pr.status);

    let days_old = (Utc::now() - pr.created).num_days();
    let age = if days_old == 0 {
        "today".to_string()
    } else if days_old == 1 {
        "1d".to_string()
    } else {
        format!("{days_old}d")
    };

    // Nerd Font CI status glyphs (requires a Nerd Font in the terminal).
    let (ci_sym, ci_color) = match pr.ci {
        CiState::Success => ("\u{f058}", theme.success), //  check-circle
        CiState::Failed => ("\u{f057}", theme.error),    //  times-circle
        CiState::Pending => ("\u{f017}", theme.warning), //  clock
        CiState::Unknown => ("\u{f042}", theme.muted),   //  adjust (half circle — neutral/not run)
    };

    // `+N`/`-N` are separate colored spans, so the padding is computed by hand.
    let plus = format!("+{}", pr.additions);
    let minus = format!("-{}", pr.deletions);
    let diff_visible = plus.chars().count() + 1 + minus.chars().count();
    let diff_pad = widths.diff.saturating_sub(diff_visible);

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

    let title = format::truncate_ellipsis(&pr.title, widths.title.saturating_sub(2));
    let author = format::truncate_ellipsis(&pr.author.username, widths.author.saturating_sub(2));

    let line = Line::from(vec![
        Span::styled(
            format!("#{:<w$}", pr.id, w = widths.id - 1),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            format!("{:<w$}", pr.status.label(), w = widths.status),
            Style::default().fg(status_color),
        ),
        Span::styled(
            format!("{:<w$}", author, w = widths.author),
            Style::default().fg(theme.info),
        ),
        Span::raw(format!("{:<w$}", title, w = widths.title)),
        Span::styled(
            format!("{:<w$}", ci_sym, w = widths.ci),
            Style::default().fg(ci_color),
        ),
        Span::styled(plus, Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(minus, Style::default().fg(theme.diff_removed)),
        Span::raw(" ".repeat(diff_pad)),
        Span::styled(
            format!("{:<w$}", comm_text, w = widths.comments),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            format!("{:<w$}", rev_text, w = widths.reviews),
            Style::default().fg(rev_color),
        ),
        Span::styled(
            format!("{:<w$}", age, w = widths.age),
            Style::default().fg(theme.muted),
        ),
    ]);
    ListItem::new(line)
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

    // `/` search (open + typing) is handled generically in the dispatcher; here
    // we only handle navigation/commands, which work the same whether or not a
    // search filter is active.
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
