use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::{
    app::state::{AppState, LoadState},
    domain::pr::PullRequest,
    tui::{Action, spinner_frame},
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
        DetailTab::Overview => render_overview(frame, pr, area),
        DetailTab::Diff => render_diff(frame, pr, state, area),
        DetailTab::Commits => render_commits(frame, pr, state, area),
        DetailTab::Checks => render_placeholder(frame, "Checks — TODO", area),
    }
}

fn render_overview(frame: &mut Frame, pr: &PullRequest, area: Rect) {
    let body = pr
        .description
        .clone()
        .unwrap_or_else(|| "(ingen beskrivning)".to_string());

    let paragraph = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

fn render_diff(frame: &mut Frame, pr: &PullRequest, state: &AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Diff ");
    let diff_state = state.cache.details.get(&pr.id).map(|d| &d.diff);

    match diff_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{} Loading diff...", spinner_frame()))
                .style(Style::default().fg(Color::Yellow))
                .block(block);
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) if diff.files.is_empty() => {
            let paragraph = Paragraph::new("(no diff)")
                .style(Style::default().fg(Color::DarkGray))
                .block(block);
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) => {
            let mut lines: Vec<Line> = Vec::new();
            for file in &diff.files {
                lines.push(Line::styled(
                    file.path.clone(),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
                for hunk in &file.hunks {
                    lines.push(Line::styled(
                        format!("@@ -{} +{} @@", hunk.old_start, hunk.new_start),
                        Style::default().fg(Color::Cyan),
                    ));
                    for diff_line in &hunk.lines {
                        let (prefix, content, color) = match diff_line {
                            crate::domain::diff::DiffLine::Added(c) => {
                                ("+", c.as_str(), Color::Green)
                            }
                            crate::domain::diff::DiffLine::Removed(c) => {
                                ("-", c.as_str(), Color::Red)
                            }
                            crate::domain::diff::DiffLine::Context(c) => {
                                (" ", c.as_str(), Color::Reset)
                            }
                        };
                        lines.push(Line::styled(
                            format!("{}{}", prefix, content),
                            Style::default().fg(color),
                        ));
                    }
                }
                lines.push(Line::raw(""));
            }
            let paragraph = Paragraph::new(lines).block(block);
            frame.render_widget(paragraph, area);
        }
    }
}

fn render_placeholder(frame: &mut Frame, text: &str, area: Rect) {
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

fn render_commits(frame: &mut Frame, pr: &PullRequest, state: &AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Commits ");
    let commits_state = state.cache.details.get(&pr.id).map(|d| &d.commits);

    match commits_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{}  Loading commits...", spinner_frame()))
                .style(Style::default().fg(Color::Yellow))
                .block(block);
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) if commits.is_empty() => {
            let paragraph = Paragraph::new("(no commits)")
                .style(Style::default().fg(Color::DarkGray))
                .block(block);
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) => {
            let items: Vec<ListItem> = commits
                .iter()
                .map(|c| {
                    let short_oid: String = c.oid.chars().take(7).collect();
                    let line = Line::from(vec![
                        Span::styled(
                            format!("{}  ", short_oid),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::raw(c.headline.clone()),
                        Span::raw("  "),
                        Span::styled(
                            format!("— {}", c.author_name),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items).block(block);
            frame.render_widget(list, area);
        }
    }
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  h/l: switch tab  esc: back  q: quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

pub fn key_to_action(_state: &AppState, key: KeyCode) -> Option<Action> {
    match key {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => Some(Action::NextTab),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => Some(Action::PrevTab),
        _ => None,
    }
}
