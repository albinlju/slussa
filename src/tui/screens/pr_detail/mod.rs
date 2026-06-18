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
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
};

use crate::{
    app::state::{
        AppState, CommentDraft, CommentTarget, ConfirmKind, DetailTab, DiffFocus, LoadState,
        PrData, SearchState, UiMemory,
    },
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
    let (main_area, footer_area, help_area) = if state.ui.help_open {
        let [main, footer, help] = layout::split(
            area,
            Direction::Vertical,
            [
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Percentage(30),
            ],
        );
        (main, footer, Some(help))
    } else {
        let [main, footer] = layout::split(
            area,
            Direction::Vertical,
            [Constraint::Min(0), Constraint::Length(1)],
        );
        (main, footer, None)
    };

    let outer = Block::default()
        .borders(Borders::ALL)
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
    let own_pr = state.viewing_own_pr(pr_id);
    let pr_data = state.cache.details.get(&pr.id);
    render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, content_area);
    render_footer_bar(frame, state, pr_data, tab, own_pr, footer_area);

    if let Some(help_area) = help_area {
        render_help_panel(frame, help_area);
    }
    if let Some(kind) = state.ui.confirm {
        render_confirm_box(frame, kind, state.ui.confirm_cursor, area);
    }
    if let Some(msg) = &state.ui.error {
        render_error_box(frame, msg, area);
    }
}

fn render_error_box(frame: &mut Frame, message: &str, area: Rect) {
    let theme = theme::current();
    let popup_w = 60.min(area.width.saturating_sub(4)).max(20);
    let text_w = popup_w.saturating_sub(4).max(1);
    let wrapped = (message.chars().count() as u16).div_ceil(text_w);
    let popup_h = (wrapped + 4).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_w) / 2,
        y: area.y + area.height.saturating_sub(popup_h) / 2,
        width: popup_w,
        height: popup_h,
    };

    let lines = vec![
        Line::from(Span::styled(
            message.to_string(),
            Style::default().fg(theme.fg),
        )),
        Line::default(),
        Line::from(Span::styled(
            "any key to dismiss",
            Style::default().fg(theme.muted),
        )),
    ];

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Error ")
        .border_style(Style::default().fg(theme.error))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_footer_bar(
    frame: &mut Frame,
    state: &AppState,
    pr_data: Option<&PrData>,
    tab: DetailTab,
    own_pr: bool,
    area: Rect,
) {
    let line = if let Some(draft) = &state.ui.comment_draft {
        comment_prompt(draft)
    } else if state.ui.comment_pending {
        widgets::loading("posting comment…")
    } else if let Some(search) = active_search(state, pr_data, tab, area.width) {
        search
    } else {
        widgets::footer(
            area.width,
            &footer_actions(state, tab, own_pr),
            state.ui.refreshing,
        )
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn footer_actions(state: &AppState, tab: DetailTab, own_pr: bool) -> String {
    // Overview is the conversation tab and home of the PR-level actions: `c`
    // posts a PR comment, `r` replies to the focused thread, `a` approves.
    // (merge will join these here once it's wired.)
    if tab == DetailTab::Overview {
        let mut parts = vec!["c: comment"];
        if state.reply_target().is_some() {
            parts.push("r: reply");
        }
        if state.editable_selected().is_some() {
            parts.push("e: edit");
            parts.push("d: delete");
        }
        if !own_pr {
            parts.push("a: approve");
        }
        return parts.join("  ");
    }
    // Other tabs: only the line-comment hint, shown once you're on a row in the
    // diff pane (a thread turns it into a reply).
    match state.comment_target() {
        Some(CommentTarget::Reply(_)) => "c: reply".to_owned(),
        Some(_) => "c: comment".to_owned(),
        None => String::new(),
    }
}

fn comment_prompt(draft: &CommentDraft) -> Line<'static> {
    let theme = theme::current();
    let label = match &draft.target {
        CommentTarget::Line(a) => format!("  comment {}:{} ▏ ", a.path, a.line),
        CommentTarget::Pr => "  comment ▏ ".to_owned(),
        CommentTarget::Reply(_) => "  reply ▏ ".to_owned(),
        CommentTarget::Edit { .. } => "  edit ▏ ".to_owned(),
    };
    Line::from(vec![
        Span::styled(label, Style::default().fg(theme.muted)),
        Span::styled(draft.text.clone(), Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ])
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
            diff::render(
                frame,
                diff,
                threads,
                &mut ui.diff,
                &pr.author.username,
                inset,
            );
        }
        DetailTab::Commits => {
            if ui.commits.open_commit.is_some() {
                let threads = activity_threads(pr_data);
                commits::render_commit_diff(
                    frame,
                    pr_data,
                    threads,
                    &mut ui.commits,
                    &pr.author.username,
                    inset,
                );
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

const HELP_KEYS: &[(&str, &str)] = &[
    ("j/k", "move up/down"),
    ("^d/^u", "half-page"),
    ("h/l", "tab / pane / fold"),
    ("1-5", "select tab"),
    ("enter", "open / view"),
    ("space", "toggle fold"),
    ("/", "search"),
    ("n/N", "next/prev match"),
    ("[ ]", "prev/next tab/commit"),
    ("esc", "back"),
    ("a", "approve"),
    ("c", "comment"),
    ("r", "reply"),
    ("^j/^k", "step comment"),
    ("e", "edit own"),
    ("d", "delete own"),
    ("R", "resolve thread"),
    ("F", "refresh"),
    ("?", "toggle help"),
    ("q", "quit"),
];

fn render_help_panel(frame: &mut Frame, area: Rect) {
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Help ")
        .border_style(Style::default().fg(theme.accent))
        .padding(Padding::symmetric(1, 1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let key_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.muted);
    let key_w = HELP_KEYS.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let desc_w = HELP_KEYS.iter().map(|(_, d)| d.len()).max().unwrap_or(0);
    let rows = (inner.height as usize).clamp(1, HELP_KEYS.len());
    let cols = HELP_KEYS.len().div_ceil(rows);

    let lines: Vec<Line<'static>> = (0..rows)
        .map(|r| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for c in 0..cols {
                if let Some((k, d)) = HELP_KEYS.get(c * rows + r) {
                    spans.push(Span::styled(format!("{k:<key_w$}  "), key_style));
                    spans.push(Span::styled(format!("{d:<w$}", w = desc_w + 3), desc_style));
                }
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

const CONFIRM_OPTIONS: [&str; 2] = ["Yes", "No"];

fn render_confirm_box(frame: &mut Frame, kind: ConfirmKind, cursor: usize, area: Rect) {
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let mut lines = vec![
        Line::from(Span::styled(kind.prompt(), Style::default().fg(theme.fg))),
        Line::default(),
    ];
    for (i, label) in CONFIRM_OPTIONS.iter().enumerate() {
        let marker = if i == cursor { "▶ " } else { "  " };
        let style = if i == cursor { selected } else { normal };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!(" {label} "), style),
        ]));
    }

    let content_w = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
    let popup_w = (content_w + 4).min(area.width);
    let popup_h = (lines.len() as u16 + 2).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_w) / 2,
        y: area.y + area.height.saturating_sub(popup_h) / 2,
        width: popup_w,
        height: popup_h,
    };

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Confirm ")
        .border_style(Style::default().fg(theme.accent))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lines), inner);
}
