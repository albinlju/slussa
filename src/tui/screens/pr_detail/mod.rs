pub mod checks;
pub mod comment;
pub mod commits;
pub mod description;
pub mod diff;
pub mod keys;
pub mod overview;

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
    app::state::{AppState, DetailTab, DiffFocus, LoadState, PrData, UiMemory},
    domain::{comment::ReviewThread, diff::FileDiff, pr::PullRequest},
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

    pub fn icon(self) -> &'static str {
        match self {
            Self::Description => "\u{f15c}", //  file-text
            Self::Overview => "\u{f086}",    //  comments
            Self::Diff => "\u{f440}",        //  diff
            Self::Commits => "\u{f417}",     //  git-commit
            Self::Builds => "\u{f085}",      //  cogs
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
    let [main_area, footer_area] =
        layout::split(area, Direction::Vertical, [Constraint::Min(0), Constraint::Length(1)]);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(main_area);
    frame.render_widget(outer, main_area);

    let [header_area, _gap, content_area] = layout::split(
        inner,
        Direction::Vertical,
        [Constraint::Length(3), Constraint::Length(1), Constraint::Min(0)],
    );

    render_header(frame, pr, header_area);
    let drilled = state.ui.commits.drilled.is_some();
    let help_focus = state.ui.active_diff_view().focus;
    let pr_data = state.cache.details.get(&pr.id);
    let footer = detail_footer(state, pr_data, tab, drilled, help_focus, footer_area.width);
    render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, content_area);
    render_help(frame, tab, drilled, help_focus, footer, footer_area);
}

fn detail_footer(
    state: &AppState,
    pr_data: Option<&PrData>,
    tab: DetailTab,
    drilled: bool,
    focus: DiffFocus,
    width: u16,
) -> Option<Line<'static>> {
    let theme = theme::current();

    if tab == DetailTab::Commits && !drilled {
        let s = &state.ui.commits.search;
        return s.open.then(|| {
            let count = match pr_data.map(|d| &d.commits) {
                Some(LoadState::Loaded(cs)) => s.filter_commits(cs).len(),
                _ => 0,
            };
            widgets::search_prompt(&s.query, count, width)
        });
    }

    if tab != DetailTab::Diff && !(tab == DetailTab::Commits && drilled) {
        return None;
    }
    let view = state.ui.active_diff_view();

    match focus {
        DiffFocus::Tree => {
            let s = &view.tree_search;
            s.open.then(|| {
                let files = active_files(state, pr_data, drilled);
                let count = files.iter().filter(|f| s.matches(&f.path)).count();
                widgets::search_prompt(&s.query, count, width)
            })
        }
        DiffFocus::Pane => {
            let s = &view.pane_search;
            if s.open {
                Some(Line::from(widgets::search_input_spans(&s.query)))
            } else if !s.query.is_empty() {
                let n = view.pane_matches.len();
                let label = if n == 1 {
                    "1 match".to_string()
                } else {
                    format!("{n} matches")
                };
                let left = vec![Span::styled(
                    format!("  /{}", s.query),
                    Style::default().fg(theme.muted),
                )];
                let right = vec![Span::styled(
                    format!("{label}   n/N: navigate   esc: clear  "),
                    Style::default().fg(theme.muted),
                )];
                Some(Line::from(widgets::justify_between(left, right, width as usize)))
            } else {
                None
            }
        }
    }
}

fn active_files<'a>(state: &AppState, pr_data: Option<&'a PrData>, drilled: bool) -> &'a [FileDiff] {
    let diff_state = if drilled {
        state
            .ui
            .commits
            .drilled
            .as_deref()
            .and_then(|oid| pr_data.and_then(|d| d.commit_diffs.get(oid)))
    } else {
        pr_data.map(|d| &d.diff)
    };
    match diff_state {
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

    let [tabs_area, content_area] =
        layout::split(area, Direction::Vertical, [Constraint::Length(3), Constraint::Min(0)]);

    let tabs_block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let tabs_inner = tabs_block.inner(tabs_area);
    frame.render_widget(tabs_block, tabs_area);
    frame.render_widget(Paragraph::new(Line::from(tab_spans)), tabs_inner);

    render_content(frame, pr, pr_data, ui, tab, content_area);
}

fn render_header(frame: &mut Frame, pr: &PullRequest, area: Rect) {
    let theme = theme::current();
    let status_color = theme.status_color(&pr.status);

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
            diff::render(frame, pr_data.map(|d| &d.diff), threads, &mut ui.diff, inset);
        }
        DetailTab::Commits => {
            if ui.commits.drilled.is_some() {
                let threads = activity_threads(pr_data);
                commits::render_commit_diff(frame, pr_data, threads, &mut ui.commits, inset);
            } else {
                commits::render(frame, pr_data, &mut ui.commits, inset);
            }
        }
        DetailTab::Builds => checks::render(frame, pr_data, inset),
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

fn render_help(
    frame: &mut Frame,
    tab: DetailTab,
    drilled: bool,
    focus: DiffFocus,
    footer: Option<Line<'static>>,
    area: Rect,
) {
    if let Some(line) = footer {
        frame.render_widget(Paragraph::new(line), area);
        return;
    }
    let hint = match (tab, drilled, focus) {
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
    };
    let line = widgets::footer(area.width, hint);
    frame.render_widget(Paragraph::new(line), area);
}
