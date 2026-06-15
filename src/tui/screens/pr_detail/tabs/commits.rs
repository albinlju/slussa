use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{
    app::state::{CommitsViewState, LoadState, PrData},
    domain::{comment::ReviewThread, commit::Commit},
    tui::{format, icons, layout, theme, widgets},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, cv: &mut CommitsViewState, area: Rect) {
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
    let last_idx = filtered.len().saturating_sub(1);
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, commit)| ListItem::new(commit_row(commit, i == last_idx, now, width)))
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(theme.highlight_bg));
    let mut list_state = ListState::default();
    list_state.select(Some(cv.selected.min(last_idx)));
    frame.render_stateful_widget(list, area, &mut list_state);
}

const COL_GAP: usize = 2;
const MIN_HEADLINE: usize = 10;

fn commit_row(commit: &Commit, is_last: bool, now: DateTime<Utc>, width: usize) -> Line<'static> {
    let theme = theme::current();
    let graph = if is_last { "└─ " } else { "├─ " };
    let age = format::relative_age(commit.authored_at, now);

    let right = vec![
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
        Span::styled(age, Style::default().fg(theme.muted)),
    ];
    let right_w: usize = right.iter().map(Span::width).sum();

    let oid_cell = format!("{}  ", short_oid(&commit.oid));
    let prefix_w = graph.chars().count() + oid_cell.chars().count();
    let headline = format::truncate_ellipsis(
        &commit.headline,
        width
            .saturating_sub(prefix_w + right_w + COL_GAP)
            .max(MIN_HEADLINE),
    );

    let left = vec![
        Span::styled(graph, Style::default().fg(theme.muted)),
        Span::styled(oid_cell, Style::default().fg(theme.accent)),
        Span::raw(headline),
    ];
    Line::from(widgets::justify_between(left, right, width))
}

pub fn render_commit_diff(
    frame: &mut Frame,
    pr_data: Option<&PrData>,
    threads: &[ReviewThread],
    cv: &mut CommitsViewState,
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

    let diff_state = super::super::active_diff(Some(&oid), pr_data);
    super::diff::render(frame, diff_state, threads, &mut cv.diff, diff_area);
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
            Style::default().fg(theme.accent),
        ),
        Span::styled(
            short_oid(oid),
            Style::default()
                .fg(theme.accent)
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

    let line = Line::from(widgets::justify_between(left, right, inner.width as usize));
    frame.render_widget(Paragraph::new(line), inner);
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(7).collect()
}
