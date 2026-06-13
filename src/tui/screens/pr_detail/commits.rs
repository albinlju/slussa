use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{
    app::state::{CommitsViewState, LoadState, PrData},
    domain::{comment::ReviewThread, commit::Commit},
    tui::{format, theme, widgets},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, cv: &mut CommitsViewState, area: Rect) {
    let theme = theme::current();
    let Some(commits) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.commits), "commits", area)
    else {
        return;
    };
    if commits.is_empty() {
        let paragraph = Paragraph::new("(no commits)").style(Style::default().fg(theme.muted));
        frame.render_widget(paragraph, area);
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
        .map(|(i, c)| ListItem::new(build_commit_line(c, i == last_idx, now, width)))
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(theme.highlight_bg));
    let mut list_state = ListState::default();
    list_state.select(Some(cv.selected.min(last_idx)));
    frame.render_stateful_widget(list, area, &mut list_state);
}

pub fn render_commit_diff(
    frame: &mut Frame,
    pr_data: Option<&PrData>,
    threads: &[ReviewThread],
    cv: &mut CommitsViewState,
    area: Rect,
) {
    let Some(oid) = cv.drilled.clone() else {
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    render_commit_banner(frame, pr_data, &oid, chunks[0]);

    let diff_state = pr_data.and_then(|d| d.commit_diffs.get(&oid));
    super::diff::render(frame, diff_state, threads, &mut cv.diff, chunks[1]);
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
    let short: String = oid.chars().take(7).collect();

    let mut left = vec![
        Span::styled("\u{f417} ", Style::default().fg(theme.accent)), //  git-commit
        Span::styled(
            short,
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some((idx, commit)) = found {
        left.push(Span::styled(
            format!("  {}/{}  ", idx + 1, total),
            Style::default().fg(theme.muted),
        ));
        left.push(Span::styled(commit.headline.clone(), Style::default().fg(theme.fg)));
    }
    let right = vec![Span::styled(
        "[ ]: prev/next   esc: list",
        Style::default().fg(theme.muted),
    )];

    let line = Line::from(widgets::justify_between(left, right, inner.width as usize));
    frame.render_widget(Paragraph::new(line), inner);
}

fn build_commit_line(c: &Commit, is_last: bool, now: DateTime<Utc>, width: usize) -> Line<'static> {
    let theme = theme::current();
    let graph = if is_last { "└─ " } else { "├─ " };
    let short_oid: String = c.oid.chars().take(7).collect();
    let age = format::relative_age(c.authored_at, now);

    let right_spans: Vec<Span<'static>> = vec![
        Span::styled(c.author_name.clone(), Style::default().fg(theme.info)),
        Span::raw("  "),
        Span::styled(
            format!("+{}", c.additions),
            Style::default().fg(theme.diff_added),
        ),
        Span::raw(" "),
        Span::styled(
            format!("-{}", c.deletions),
            Style::default().fg(theme.diff_removed),
        ),
        Span::styled("  · ", Style::default().fg(theme.muted)),
        Span::styled(age, Style::default().fg(theme.muted)),
    ];
    let right_visible: usize = right_spans.iter().map(Span::width).sum();

    let left_fixed = graph.chars().count() + short_oid.chars().count() + 2;
    let headline = format::truncate_ellipsis(
        &c.headline,
        width
            .saturating_sub(left_fixed + right_visible + 2)
            .max(10),
    );

    let used = left_fixed + Span::raw(headline.as_str()).width() + right_visible;
    let pad = width.saturating_sub(used).max(2);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(4 + right_spans.len());
    spans.push(Span::styled(graph, Style::default().fg(theme.muted)));
    spans.push(Span::styled(
        format!("{short_oid}  "),
        Style::default().fg(theme.accent),
    ));
    spans.push(Span::raw(headline));
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right_spans);
    Line::from(spans)
}
