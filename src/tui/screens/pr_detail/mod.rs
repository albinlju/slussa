mod build_status;
pub mod comment;
pub mod keys;
mod tabs;

use tabs::{builds, commits, description, diff, overview};

pub(super) use comment::render_inline_thread;
pub(in crate::tui) use keys::key_to_action;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

use crate::{
    app::state::{AppState, DetailTab, DiffFocus, LoadState, PrData, SearchState, UiMemory},
    domain::{
        comment::ReviewThread,
        diff::{Diff, FileDiff},
        pr::PullRequest,
    },
    tui::{layout, theme, widgets},
};

impl DetailTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Overview => "Overview",
            Self::Diff => "Diff",
            Self::Commits => "Commits",
            Self::Builds => "Builds",
        }
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
    let [main_area, footer_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Min(0), Constraint::Length(1)],
    );

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(main_area);
    frame.render_widget(outer, main_area);

    let [header_area, _gap, content_area] = layout::split(
        inner,
        Direction::Vertical,
        [
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ],
    );

    render_header(frame, pr, header_area);
    let pr_data = state.cache.details.get(&pr.id);
    render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, content_area);
    render_footer_bar(frame, state, pr_data, tab, footer_area);
}

fn render_footer_bar(
    frame: &mut Frame,
    state: &AppState,
    pr_data: Option<&PrData>,
    tab: DetailTab,
    area: Rect,
) {
    let line = if let Some(search) = active_search(state, pr_data, tab, area.width) {
        search
    } else {
        let viewing_commit = state.ui.commits.open_commit.is_some();
        let focus = state.ui.active_diff_view().focus;
        widgets::footer(area.width, footer_hint(tab, viewing_commit, focus))
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn active_search(
    state: &AppState,
    pr_data: Option<&PrData>,
    tab: DetailTab,
    width: u16,
) -> Option<Line<'static>> {
    let viewing_commit = state.ui.commits.open_commit.is_some();
    if tab == DetailTab::Commits && !viewing_commit {
        return commits_search_prompt(&state.ui.commits.search, pr_data, width);
    }
    if tab != DetailTab::Diff && !(tab == DetailTab::Commits && viewing_commit) {
        return None;
    }
    let view = state.ui.active_diff_view();
    match view.focus {
        DiffFocus::Tree => tree_search_prompt(&view.tree_search, tree_files(state, pr_data), width),
        DiffFocus::Pane => pane_search_prompt(&view.pane_search, view.pane_matches.len(), width),
    }
}

fn commits_search_prompt(
    search: &SearchState,
    pr_data: Option<&PrData>,
    width: u16,
) -> Option<Line<'static>> {
    search.open.then(|| {
        let count = match pr_data.map(|d| &d.commits) {
            Some(LoadState::Loaded(commits)) => search.filter_commits(commits).len(),
            _ => 0,
        };
        widgets::search_prompt(&search.query, count, width)
    })
}

fn tree_search_prompt(
    search: &SearchState,
    files: &[FileDiff],
    width: u16,
) -> Option<Line<'static>> {
    search.open.then(|| {
        let count = files.iter().filter(|f| search.matches(&f.path)).count();
        widgets::search_prompt(&search.query, count, width)
    })
}

fn pane_search_prompt(
    search: &SearchState,
    match_count: usize,
    width: u16,
) -> Option<Line<'static>> {
    if search.open {
        return Some(Line::from(widgets::search_input_spans(&search.query)));
    }
    if search.query.is_empty() {
        return None;
    }
    let theme = theme::current();
    let label = if match_count == 1 {
        "1 match".to_string()
    } else {
        format!("{match_count} matches")
    };
    let left = vec![Span::styled(
        format!("  /{}", search.query),
        Style::default().fg(theme.muted),
    )];
    let right = vec![Span::styled(
        format!("{label}   n/N: navigate   esc: clear  "),
        Style::default().fg(theme.muted),
    )];
    Some(Line::from(widgets::justify_between(
        left,
        right,
        width as usize,
    )))
}

pub(super) fn active_diff<'a>(
    open_commit: Option<&str>,
    pr_data: Option<&'a PrData>,
) -> Option<&'a LoadState<Diff>> {
    match open_commit {
        Some(oid) => pr_data.and_then(|d| d.commit_diffs.get(oid)),
        None => pr_data.map(|d| &d.diff),
    }
}

fn tree_files<'a>(state: &AppState, pr_data: Option<&'a PrData>) -> &'a [FileDiff] {
    match active_diff(state.ui.commits.open_commit.as_deref(), pr_data) {
        Some(LoadState::Loaded(diff)) => &diff.files,
        _ => &[],
    }
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
    let [tabs_area, content_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Length(3), Constraint::Min(0)],
    );

    let tabs_block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let tabs_inner = tabs_block.inner(tabs_area);
    frame.render_widget(tabs_block, tabs_area);
    frame.render_widget(Paragraph::new(tab_bar(tab)), tabs_inner);

    render_content(frame, pr, pr_data, ui, tab, content_area);
}

fn tab_bar(tab: DetailTab) -> Line<'static> {
    let theme = theme::current();
    let active = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(theme.muted);
    let sep = Style::default().fg(theme.muted);

    let mut spans = vec![Span::raw("  ")];
    for (i, t) in DetailTab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", sep));
        }
        let style = if i == tab.index() { active } else { inactive };
        spans.push(Span::styled(t.label(), style));
    }
    Line::from(spans)
}

fn render_header(frame: &mut Frame, pr: &PullRequest, area: Rect) {
    let theme = theme::current();
    let status_color = theme.status_color(&pr.status);

    let title_line = Line::from(vec![
        Span::styled(format!("#{} ", pr.id), Style::default().fg(theme.muted)),
        Span::styled(
            pr.title.clone(),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
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
        Span::styled("  wants to merge  ", Style::default().fg(theme.muted)),
        Span::styled(pr.source_branch.clone(), Style::default().fg(theme.orange)),
        Span::raw(" → "),
        Span::styled(pr.target_branch.clone(), Style::default().fg(theme.info)),
    ];
    let meta_line = Line::from(left_spans);

    let paragraph = Paragraph::new(vec![title_line, Line::default(), meta_line])
        .block(Block::default().padding(Padding::horizontal(2)));
    frame.render_widget(paragraph, area);
}

fn render_content(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    tab: DetailTab,
    area: Rect,
) {
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
        DetailTab::Overview => overview::render(frame, pr, pr_data, ui, inset),
        DetailTab::Diff => {
            let threads = activity_threads(pr_data);
            let diff = active_diff(ui.commits.open_commit.as_deref(), pr_data);
            diff::render(frame, diff, threads, &mut ui.diff, inset);
        }
        DetailTab::Commits => {
            if ui.commits.open_commit.is_some() {
                let threads = activity_threads(pr_data);
                commits::render_commit_diff(frame, pr_data, threads, &mut ui.commits, inset);
            } else {
                commits::render(frame, pr_data, &mut ui.commits, inset);
            }
        }
        DetailTab::Builds => builds::render(frame, pr_data, inset),
    }
}

fn activity_threads(pr_data: Option<&PrData>) -> &[ReviewThread] {
    pr_data
        .and_then(|d| match &d.activity {
            LoadState::Loaded(b) => Some(b.threads.as_slice()),
            _ => None,
        })
        .unwrap_or(&[])
}

fn footer_hint(tab: DetailTab, viewing_commit: bool, focus: DiffFocus) -> &'static str {
    match (tab, viewing_commit, focus) {
        (DetailTab::Diff, _, DiffFocus::Tree) => {
            "j/k: files  ^d/^u: page  enter: open  h/l: fold  esc: back"
        }
        (DetailTab::Diff, _, DiffFocus::Pane) => "j/k: line  ^d/^u: page  h/esc: tree  q: quit",
        (DetailTab::Commits, true, DiffFocus::Tree) => {
            "j/k: files  enter: open  [ ]: prev/next  esc: list"
        }
        (DetailTab::Commits, true, DiffFocus::Pane) => {
            "j/k: line  [ ]: prev/next commit  h/esc: tree"
        }
        (DetailTab::Commits, false, _) => {
            "j/k: commits  ^d/^u: page  enter: view diff  h/l: tab  esc: back"
        }
        _ => "1-5 / h/l: tab  j/k: scroll  ^d/^u: page  esc: back",
    }
}
