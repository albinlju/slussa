pub mod checks;
pub mod commits;
pub mod description;
pub mod diff;
pub mod file_tree;
pub mod overview;

use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

use crate::{
    app::{
        action::{Action, DetailAction, DiffAction},
        state::{AppState, DiffFocus, LoadState, PrData, Screen, UiMemory},
    },
    domain::{
        comment::ReviewThread,
        pr::{PrStatus, PullRequest},
        review::ReviewerState,
    },
    tui::{theme, widgets},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Description,
    Overview,
    Diff,
    Commits,
    Builds,
}

impl DetailTab {
    pub const ALL: [Self; 5] = [
        Self::Description,
        Self::Overview,
        Self::Diff,
        Self::Commits,
        Self::Builds,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Overview => "Overview",
            Self::Diff => "Diff",
            Self::Commits => "Commits",
            Self::Builds => "Builds",
        }
    }

    /// Nerd Font glyphs (requires a Nerd Font in the terminal).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Description => "\u{f15c}", //  file-text
            Self::Overview => "\u{f086}",    //  comments
            Self::Diff => "\u{f440}",        //  diff
            Self::Commits => "\u{f417}",     //  git-commit
            Self::Builds => "\u{f085}",      //  cogs
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let len = Self::ALL.len();
        Self::ALL[(self.index() + len - 1) % len]
    }
}

pub(in crate::tui) fn render(
    frame: &mut Frame,
    state: &mut AppState,
    pr_id: u64,
    tab: DetailTab,
    area: Rect,
) {
    let LoadState::Loaded(prs) = &state.cache.prs else {
        return;
    };
    let Some(pr) = prs.iter().find(|p| p.id == pr_id) else {
        return;
    };

    let theme = theme::current();
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(outer_chunks[0]);
    frame.render_widget(outer, outer_chunks[0]);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header (title + blank + meta)
            Constraint::Length(1), // spacer between header and tabs
            Constraint::Min(0),    // tabs + divider + content
        ])
        .split(inner);

    render_header(frame, pr, chunks[0]);
    let diff_focus = state.ui.diff.focus;
    let pr_data = state.cache.details.get(&pr.id);
    render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, chunks[2]);
    render_help(frame, tab, diff_focus, outer_chunks[1]);
}

pub(super) fn description_body(pr: &PullRequest) -> &str {
    pr.description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("(ingen beskrivning)")
}

/// Render a review thread as a `┃`-bar-prefixed block. Shared between the
/// inline diff view and the Overview tab so review comments look identical in
/// both places.
pub(super) fn render_inline_thread(
    thread: &ReviewThread,
    width: u16,
    now: DateTime<Utc>,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let text_w = widgets::box_text_width(width);

    // Box header: status icon + label on the left, comment count on the right.
    let (icon, label, accent) = if thread.resolved {
        ("\u{f058}", "Resolved conversation", theme.success) //  check-circle
    } else {
        ("\u{f071}", "Unresolved", theme.warning) //  exclamation-triangle
    };
    let count = thread.comments.len();
    let count_label = if count == 1 {
        "1 comment".to_string()
    } else {
        format!("{count} comments")
    };

    let left = vec![
        Span::styled(
            icon,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            label,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ];
    let right = vec![Span::styled(count_label, Style::default().fg(theme.muted))];
    let header = Line::from(widgets::justify_between(left, right, text_w as usize));

    let mut body: Vec<Line<'static>> = Vec::new();
    for (i, comment) in thread.comments.iter().enumerate() {
        if i > 0 {
            body.push(Line::raw(""));
        }
        let age = widgets::relative_age(comment.created, now);
        body.push(Line::from(vec![
            Span::styled(
                comment.author.username.clone(),
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" · {age}"), Style::default().fg(theme.muted)),
        ]));
        body.extend(widgets::trim_blank_lines(widgets::strip_glamour_margin(
            widgets::markdown(&comment.content, text_w + 2),
            2,
        )));
    }

    widgets::boxed(header, body, width)
}

fn render_tabs_and_content(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    tab: DetailTab,
    area: Rect,
) {
    let theme = theme::current();
    let active_idx = tab.index();
    let active_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(theme.muted);
    let sep_style = Style::default().fg(theme.muted);

    let mut tab_spans: Vec<Span<'static>> = Vec::new();
    tab_spans.push(Span::raw("  "));
    for (i, t) in DetailTab::ALL.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" · ", sep_style));
        }
        let style = if i == active_idx {
            active_style
        } else {
            inactive_style
        };
        tab_spans.push(Span::styled(format!("{}  {}", t.icon(), t.label()), style));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // top divider + tabs row + bottom divider
            Constraint::Min(0),    // content (outer frame handles the border)
        ])
        .split(area);

    // Tabs sit sandwiched between two divider lines so the row reads as its
    // own band, separated from both the header above and the content below.
    let tabs_block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let tabs_inner = tabs_block.inner(chunks[0]);
    frame.render_widget(tabs_block, chunks[0]);
    frame.render_widget(Paragraph::new(Line::from(tab_spans)), tabs_inner);

    render_content(frame, pr, pr_data, ui, tab, chunks[1]);
}

fn render_header(frame: &mut Frame, pr: &PullRequest, area: Rect) {
    let theme = theme::current();
    let status_color = match pr.status {
        PrStatus::Draft => theme.status_draft,
        PrStatus::Open => theme.status_open,
        PrStatus::Merged => theme.status_merged,
        PrStatus::Declined => theme.status_declined,
    };

    let title_line = Line::from(vec![
        Span::styled(format!("#{} ", pr.id), Style::default().fg(theme.muted)),
        Span::styled(
            pr.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);

    let left_spans: Vec<Span<'static>> = vec![
        Span::styled("\u{e0b6}", Style::default().fg(status_color)),
        Span::styled(
            pr.status.label().to_string(),
            Style::default()
                .fg(theme.bg)
                .bg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("\u{e0b4}", Style::default().fg(status_color)),
        Span::styled(
            format!(" @{}", pr.author.username),
            Style::default().fg(theme.info),
        ),
        Span::raw("  wants to merge  "),
        Span::styled(pr.source_branch.clone(), Style::default().fg(theme.orange)),
        Span::raw(" → "),
        Span::styled(pr.target_branch.clone(), Style::default().fg(theme.accent)),
    ];
    let right_spans = build_reviewer_spans(pr);

    let content_width = (area.width as usize).saturating_sub(4);
    let left_visible: usize = left_spans.iter().map(|s| s.width()).sum();
    let right_visible: usize = right_spans.iter().map(|s| s.width()).sum();
    let gap = content_width
        .saturating_sub(left_visible + right_visible)
        .max(1);

    let mut meta_spans = left_spans;
    if !right_spans.is_empty() {
        meta_spans.push(Span::raw(" ".repeat(gap)));
        meta_spans.extend(right_spans);
    }
    let meta_line = Line::from(meta_spans);

    let paragraph = Paragraph::new(vec![title_line, Line::default(), meta_line])
        .block(Block::default().padding(Padding::horizontal(2)));
    frame.render_widget(paragraph, area);
}

fn build_reviewer_spans(pr: &PullRequest) -> Vec<Span<'static>> {
    let theme = theme::current();
    if pr.reviewers.is_empty() {
        return Vec::new();
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, reviewer) in pr.reviewers.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let (icon, color) = match reviewer.state {
            ReviewerState::Approved => ("\u{f058}", theme.success), //  check-circle
            ReviewerState::ChangesRequested => ("\u{f057}", theme.error), //  times-circle
            ReviewerState::Commented => ("\u{f075}", theme.info),   //  comment
        };
        spans.push(Span::styled(
            format!("@{}", reviewer.author.username),
            Style::default().fg(theme.info),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(icon, Style::default().fg(color)));
    }
    spans
}

fn render_content(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    tab: DetailTab,
    area: Rect,
) {
    // Description renders through glamour, which adds its own ~2-col
    // left/right margins. The other tabs render text directly, so we inset
    // their area on both sides to match the visual indent.
    let inset = match tab {
        DetailTab::Description => area,
        _ => Rect {
            x: area.x + 2,
            y: area.y,
            width: area.width.saturating_sub(4),
            height: area.height,
        },
    };
    match tab {
        DetailTab::Description => description::render(frame, pr, ui, inset),
        DetailTab::Overview => overview::render(frame, pr_data, ui, inset),
        DetailTab::Diff => diff::render(frame, pr_data, &mut ui.diff, inset),
        DetailTab::Commits => commits::render(frame, pr_data, inset),
        DetailTab::Builds => checks::render(frame, pr_data, inset),
    }
}

fn render_help(frame: &mut Frame, tab: DetailTab, diff_focus: DiffFocus, area: Rect) {
    let hint = match (tab, diff_focus) {
        (DetailTab::Diff, DiffFocus::Tree) => {
            "j/k: files  enter: open diff  h/l: fold  tab: section  esc: back"
        }
        (DetailTab::Diff, DiffFocus::Pane) => {
            "j/k: scroll  h/esc: tree  tab: section  q: quit"
        }
        _ => "1-5 / h/l: tab  j/k: scroll  esc: back  q: quit",
    };
    let line = widgets::footer(area.width, hint);
    frame.render_widget(Paragraph::new(line), area);
}

pub(in crate::tui) fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    if matches!(key, KeyCode::Char('q')) {
        return Some(Action::Quit);
    }

    let tab = match state.screen {
        Screen::Detail { tab, .. } => tab,
        _ => return None,
    };
    let diff_focus = state.ui.diff.focus;

    // Esc steps back one level: from the diff pane to the tree, otherwise out
    // of the detail view entirely.
    if matches!(key, KeyCode::Esc) {
        if tab == DetailTab::Diff && diff_focus == DiffFocus::Pane {
            return Some(Action::Diff(DiffAction::FocusTree));
        }
        return Some(Action::Detail(DetailAction::Back));
    }

    match key {
        KeyCode::Tab => return Some(Action::Detail(DetailAction::NextTab)),
        KeyCode::BackTab => return Some(Action::Detail(DetailAction::PrevTab)),
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            if let Some(&t) = DetailTab::ALL.get(idx) {
                return Some(Action::Detail(DetailAction::SelectTab(t)));
            }
        }
        _ => {}
    }

    match tab {
        // Tree focus: navigate files, Enter jumps into the pane.
        DetailTab::Diff if diff_focus == DiffFocus::Tree => match key {
            KeyCode::Down | KeyCode::Char('j') => Some(Action::Diff(DiffAction::CursorDown)),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Diff(DiffAction::CursorUp)),
            KeyCode::Enter => Some(Action::Diff(DiffAction::EnterPane)),
            KeyCode::Char(' ') => Some(Action::Diff(DiffAction::ToggleAtCursor)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Diff(DiffAction::CollapseAtCursor)),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Diff(DiffAction::ExpandAtCursor)),
            _ => None,
        },
        // Pane focus: scroll the diff; Enter/h/Left hand focus back to the tree.
        DetailTab::Diff => match key {
            KeyCode::Down | KeyCode::Char('j') => Some(Action::Diff(DiffAction::PaneScrollDown)),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Diff(DiffAction::PaneScrollUp)),
            KeyCode::PageDown | KeyCode::Char('J') => {
                Some(Action::Diff(DiffAction::PaneScrollDown))
            }
            KeyCode::PageUp | KeyCode::Char('K') => Some(Action::Diff(DiffAction::PaneScrollUp)),
            KeyCode::Enter | KeyCode::Left | KeyCode::Char('h') => {
                Some(Action::Diff(DiffAction::FocusTree))
            }
            _ => None,
        },
        DetailTab::Overview => match key {
            KeyCode::Down | KeyCode::Char('j') => {
                Some(Action::Detail(DetailAction::OverviewScrollDown))
            }
            KeyCode::Up | KeyCode::Char('k') => {
                Some(Action::Detail(DetailAction::OverviewScrollUp))
            }
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
        DetailTab::Description => match key {
            KeyCode::Down | KeyCode::Char('j') => {
                Some(Action::Detail(DetailAction::DescriptionScrollDown))
            }
            KeyCode::Up | KeyCode::Char('k') => {
                Some(Action::Detail(DetailAction::DescriptionScrollUp))
            }
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
        _ => match key {
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
    }
}
