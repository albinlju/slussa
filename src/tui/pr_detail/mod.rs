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
    widgets::{Block, Borders, Paragraph},
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
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, pr, chunks[0]);
    render_tabs_and_content(frame, pr, state, tab, chunks[1]);
    render_help(frame, chunks[2]);
}

fn render_tabs_and_content(
    frame: &mut Frame,
    pr: &PullRequest,
    state: &AppState,
    tab: DetailTab,
    area: Rect,
) {
    let labels: Vec<&str> = DetailTab::ALL.iter().map(|t| t.label()).collect();
    let active_idx = tab.index();

    let cell_widths: Vec<usize> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            if i == active_idx {
                label.chars().count() + 4
            } else {
                label.chars().count() + 2
            }
        })
        .collect();

    let starts: Vec<usize> = {
        let mut s = Vec::with_capacity(labels.len());
        let mut col = 0;
        for w in &cell_widths {
            s.push(col);
            col += w;
        }
        s
    };

    let area_w = area.width as usize;
    let active_start = starts[active_idx];
    let active_width = cell_widths[active_idx];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let header_lines: Vec<Line> = vec![
        Line::raw(build_top_row(area_w, active_start, active_width)),
        build_label_row(&labels, active_idx),
        Line::raw(build_join_row(area_w, active_start, active_width)),
    ];
    let header = Paragraph::new(header_lines);
    frame.render_widget(header, chunks[0]);

    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM);
    let content_inner = content_block.inner(chunks[1]);
    frame.render_widget(content_block, chunks[1]);

    render_content(frame, pr, state, tab, content_inner);
}

fn build_top_row(area_w: usize, active_start: usize, active_width: usize) -> String {
    let active_end = active_start + active_width.saturating_sub(1);
    let mut s = String::with_capacity(area_w);
    for col in 0..area_w {
        let c = if col == active_start {
            '┌'
        } else if col == active_end {
            '┐'
        } else if col > active_start && col < active_end {
            '─'
        } else {
            ' '
        };
        s.push(c);
    }
    s
}

fn build_label_row(labels: &[&str], active_idx: usize) -> Line<'static> {
    let active_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(Color::DarkGray);

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if i == active_idx {
            spans.push(Span::raw("│ "));
            spans.push(Span::styled(label.to_string(), active_style));
            spans.push(Span::raw(" │"));
        } else {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(label.to_string(), inactive_style));
            spans.push(Span::raw(" "));
        }
    }
    Line::from(spans)
}

fn build_join_row(area_w: usize, active_start: usize, active_width: usize) -> String {
    let active_end = active_start + active_width.saturating_sub(1);
    let last_col = area_w.saturating_sub(1);

    let mut s = String::with_capacity(area_w);
    for col in 0..area_w {
        let c = if col == active_start && col == 0 {
            '│'
        } else if col == active_end && col == last_col {
            '│'
        } else if col == active_start {
            '┘'
        } else if col == active_end {
            '└'
        } else if col > active_start && col < active_end {
            ' '
        } else if col == 0 {
            '┌'
        } else if col == last_col {
            '┐'
        } else {
            '─'
        };
        s.push(c);
    }
    s
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
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
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
