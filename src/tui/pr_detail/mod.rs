pub mod checks;
pub mod commits;
pub mod diff;
pub mod file_tree;
pub mod overview;

use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::{
    app::state::{AppState, LoadState},
    domain::pr::PullRequest,
    tui::Action,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Overview,
    Diff,
    Commits,
    Checks,
}

impl DetailTab {
    pub const ALL: [Self; 4] = [Self::Overview, Self::Diff, Self::Commits, Self::Checks];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Diff => "Diff",
            Self::Commits => "Commits",
            Self::Checks => "Checks",
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

pub fn render(frame: &mut Frame, state: &AppState, pr_id: u64, tab: DetailTab, area: Rect) {
    let prs = match &state.cache.prs {
        LoadState::Loaded(prs) => prs,
        _ => return,
    };
    let Some(pr) = prs.iter().find(|p| p.id == pr_id) else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, pr, chunks[0]);
    render_tabs(frame, tab, chunks[1]);
    render_content(frame, pr, state, tab, chunks[2]);
    render_help(frame, chunks[3]);
}

fn render_header(frame: &mut Frame, pr: &PullRequest, area: Rect) {
    let title_line = Line::from(vec![
        Span::styled(format!("#{} ", pr.id), Style::default().fg(Color::DarkGray)),
        Span::styled(
            pr.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);

    let meta_line = Line::from(vec![
        Span::styled(
            format!("@{}", pr.author.username),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  wants to merge  "),
        Span::styled(pr.source_branch.clone(), Style::default().fg(Color::Green)),
        Span::raw(" → "),
        Span::styled(pr.target_branch.clone(), Style::default().fg(Color::Yellow)),
    ]);

    let paragraph = Paragraph::new(vec![title_line, meta_line]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(paragraph, area);
}

fn render_tabs(frame: &mut Frame, current: DetailTab, area: Rect) {
    let titles: Vec<Line> = DetailTab::ALL
        .iter()
        .map(|t| Line::from(t.label()))
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL))
        .select(current.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

fn render_content(
    frame: &mut Frame,
    pr: &PullRequest,
    state: &AppState,
    tab: DetailTab,
    area: Rect,
) {
    match tab {
        DetailTab::Overview => overview::render(frame, pr, area),
        DetailTab::Diff => diff::render(frame, pr, state, area),
        DetailTab::Commits => commits::render(frame, pr, state, area),
        DetailTab::Checks => checks::render(frame, area),
    }
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  h/l: switch tab  esc: back  q: quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

pub(super) fn render_placeholder(frame: &mut Frame, text: &str, area: Rect) {
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    match key {
        KeyCode::Char('q') => return Some(Action::Quit),
        KeyCode::Esc => return Some(Action::Back),
        KeyCode::Tab => return Some(Action::NextTab),
        KeyCode::BackTab => return Some(Action::PrevTab),
        _ => {}
    }

    let tab = match state.screen {
        crate::app::state::Screen::Detail { tab, .. } => tab,
        _ => return None,
    };

    match tab {
        DetailTab::Diff => match key {
            KeyCode::Down | KeyCode::Char('j') => Some(Action::DiffCursorDown),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::DiffCursorUp),
            KeyCode::Enter | KeyCode::Char(' ') => Some(Action::DiffToggleAtCursor),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::DiffCollapseAtCursor),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::DiffExpandAtCursor),
            _ => None,
        },
        _ => match key {
            KeyCode::Right | KeyCode::Char('l') => Some(Action::NextTab),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::PrevTab),
            _ => None,
        },
    }
}
