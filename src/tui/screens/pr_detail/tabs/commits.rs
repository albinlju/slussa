use crate::{
    app::{
        action::{Action, CommitsAction},
        reviews::PendingComment,
        store::{LoadState, PrData},
    },
    domain::{comment::CommentThread, commit::Commit},
    tui::{
        component::{Component, step_index},
        components::{
            diff_viewer::{DiffContext, DiffViewer},
            search_input::SearchInput,
        },
        format, icons, layout, theme, widgets,
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, cv: &mut CommitList, area: Rect) {
    let theme = theme::current();
    let Some(commits) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.commits), "commits", area)
    else {
        return;
    };
    if commits.is_empty() {
        frame.render_widget(widgets::empty_state("(no commits)"), area);
        return;
    }

    cv.viewport = area.height;
    let now = Utc::now();
    let width = area.width as usize;
    let filtered: Vec<&Commit> = cv.search.filter_commits(commits);
    if filtered.is_empty() {
        frame.render_widget(
            widgets::empty_state("No matching commits. Esc clears search."),
            area,
        );
        return;
    }
    let last_idx = filtered.len().saturating_sub(1);
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, commit)| ListItem::new(commit_row(commit, i == last_idx, now, width)))
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(theme.highlight_bg));
    cv.selected = cv.selected.min(last_idx);
    cv.list_state.select(Some(cv.selected));
    frame.render_stateful_widget(list, area, &mut cv.list_state);
}

fn commit_row(commit: &Commit, is_last: bool, now: DateTime<Utc>, width: usize) -> Line<'static> {
    let theme = theme::current();
    let graph = if is_last { "└─ " } else { "├─ " };
    let age = format::relative_age(commit.authored_at, now);

    let mut right = vec![
        Span::styled(commit.author_name.clone(), Style::default().fg(theme.info)),
        Span::raw("  "),
        Span::styled(
            format!("+{}", commit.additions),
            Style::default().fg(theme.diff_added),
        ),
        Span::raw(" "),
        Span::styled(
            format!("-{}", commit.deletions),
            Style::default().fg(theme.diff_removed),
        ),
        Span::styled("  · ", Style::default().fg(theme.muted)),
        Span::styled(age.clone(), Style::default().fg(theme.muted)),
    ];
    if width < 80 {
        right = vec![Span::styled(age, Style::default().fg(theme.muted))];
    }
    if width < 45 {
        right.clear();
    }
    let oid_cell = format!("{}  ", short_oid(&commit.oid));
    let headline = commit.headline.clone();

    let left = vec![
        Span::styled(graph, Style::default().fg(theme.muted)),
        Span::styled(oid_cell, Style::default().fg(theme.decorative)),
        Span::raw(headline),
    ];
    widgets::fitted_row(left, right, width)
}

pub fn render_commit_diff(
    frame: &mut Frame,
    pr_data: Option<&PrData>,
    threads: &[CommentThread],
    pending: &[PendingComment],
    cv: &mut CommitList,
    author: &str,
    area: Rect,
) {
    let Some(oid) = cv.open_commit.clone() else {
        return;
    };
    let [banner_area, diff_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Length(2), Constraint::Min(0)],
    );

    render_commit_banner(frame, pr_data, &oid, banner_area);

    let diff_state = pr_data.and_then(|d| d.diff_for(Some(&oid)));
    cv.diff.render(
        frame,
        diff_area,
        &DiffContext {
            diff: diff_state,
            threads,
            pending,
            author,
        },
    );
}

fn render_commit_banner(frame: &mut Frame, pr_data: Option<&PrData>, oid: &str, area: Rect) {
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let commits = match pr_data.map(|d| &d.commits) {
        Some(LoadState::Loaded(c)) => c.as_slice(),
        _ => &[],
    };
    let found = commits.iter().enumerate().find(|(_, c)| c.oid == oid);
    let total = commits.len();

    let mut left = vec![
        Span::styled(
            format!("{} ", icons::GIT_COMMIT),
            Style::default().fg(theme.decorative),
        ),
        Span::styled(
            short_oid(oid),
            Style::default()
                .fg(theme.decorative)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some((idx, commit)) = found {
        left.push(Span::styled(
            format!("  {}/{}  ", idx + 1, total),
            Style::default().fg(theme.muted),
        ));
        left.push(Span::styled(
            commit.headline.clone(),
            Style::default().fg(theme.fg),
        ));
    }
    let right = vec![Span::styled(
        "[ ]: prev/next   esc: list",
        Style::default().fg(theme.muted),
    )];

    let right = if inner.width < 60 {
        vec![Span::styled("esc: list", Style::default().fg(theme.muted))]
    } else {
        right
    };
    let line = widgets::fitted_row(left, right, inner.width as usize);
    frame.render_widget(Paragraph::new(line), inner);
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(7).collect()
}

#[derive(Debug, Default)]
pub struct CommitList {
    pub selected: usize,
    list_state: ListState,
    pub viewport: u16,
    pub search: SearchInput,
    pub open_commit: Option<String>,
    pub diff: DiffViewer,
}

pub struct CommitContext<'a> {
    pub pr_id: u64,
    pub data: Option<&'a PrData>,
    pub pending: &'a [PendingComment],
    pub author: &'a str,
}
impl Component for CommitList {
    type Context<'a> = CommitContext<'a>;
    type Message = CommitsAction;
    fn handle_key(&self, key: KeyEvent, _: &CommitContext<'_>) -> Option<Action> {
        let action = match key.code {
            KeyCode::Char('j') | KeyCode::Down => CommitsAction::MoveSelection(1),
            KeyCode::Char('k') | KeyCode::Up => CommitsAction::MoveSelection(-1),
            KeyCode::PageDown => {
                CommitsAction::MoveSelection(crate::tui::screens::half_page(self.viewport))
            }
            KeyCode::PageUp => {
                CommitsAction::MoveSelection(-crate::tui::screens::half_page(self.viewport))
            }
            KeyCode::Enter => CommitsAction::Open,
            _ => return None,
        };
        Some(Action::Commits(action))
    }
    fn update(&mut self, action: CommitsAction, ctx: &CommitContext<'_>) -> Option<Action> {
        let commits = match ctx.data.map(|d| &d.commits) {
            Some(LoadState::Loaded(c)) => c.as_slice(),
            _ => &[],
        };
        let filtered = self.search.filter_commits(commits);
        match action {
            CommitsAction::Back => {
                self.open_commit = None;
                return None;
            }
            CommitsAction::MoveSelection(delta) => {
                self.selected = step_index(self.selected, delta, filtered.len());
                return None;
            }
            CommitsAction::StepCommit(delta) => {
                let next = step_index(self.selected, delta, filtered.len());
                if next == self.selected {
                    return None;
                }
                self.selected = next;
            }
            CommitsAction::Open => {}
        }
        let oid = filtered.get(self.selected)?.oid.clone();
        self.open_commit = Some(oid.clone());
        self.diff = DiffViewer::default();
        Some(Action::LoadCommitDiff {
            pr_id: ctx.pr_id,
            oid,
        })
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &CommitContext<'_>) {
        if self.open_commit.is_some() {
            let threads = match ctx.data.map(|d| &d.activity) {
                Some(LoadState::Loaded(a)) => a.threads.as_slice(),
                _ => &[],
            };
            render_commit_diff(
                frame,
                ctx.data,
                threads,
                ctx.pending,
                self,
                ctx.author,
                area,
            );
        } else {
            render(frame, ctx.data, self, area);
        }
    }
}

impl CommitList {
    pub fn update_search(&mut self, action: crate::app::action::SearchAction) {
        use crate::app::action::SearchAction;
        self.search.update(
            action,
            &crate::tui::components::search_input::SearchContext {
                highlight: false,
                matches: 0,
            },
        );
        if !matches!(action, SearchAction::Open | SearchAction::Confirm) {
            self.selected = 0;
            self.list_state = ListState::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_rows_prioritize_title_in_narrow_views() {
        let now = Utc::now();
        let commit = Commit {
            oid: "abcdef123456".into(),
            headline: "Fix 非常に長い headline with more details".into(),
            author_name: "a-very-long-author-name".into(),
            authored_at: now,
            additions: 1234,
            deletions: 5678,
        };
        for width in [0, 1, 20, 40, 60, 100] {
            let line = commit_row(&commit, true, now, width);
            assert!(line.width() <= width);
            if width >= 20 {
                assert!(line.to_string().contains("abcdef1"));
            }
            if width == 40 {
                assert!(line.to_string().contains("Fix"));
                assert!(!line.to_string().contains("author"));
            }
            if width >= 80 {
                assert!(line.to_string().contains("+1234 -5678"));
            }
        }
    }
}
