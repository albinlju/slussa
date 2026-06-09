pub mod checks;
pub mod commits;
pub mod description;
pub mod diff;
pub mod file_tree;
pub mod overview;

use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

use crate::{
    app::{
        action::{Action, CommitsAction, DetailAction, DiffAction},
        state::{AppState, DiffFocus, DiffViewState, LoadState, PrData, Screen, UiMemory},
    },
    domain::{
        comment::ReviewThread,
        pr::{PrStatus, PullRequest},
    },
    tui::{screens::half_page, theme, widgets},
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
            Constraint::Length(3), // title + blank + branch meta
            Constraint::Length(1), // spacer between header and tabs
            Constraint::Min(0),    // tabs + divider + content
        ])
        .split(inner);

    render_header(frame, pr, chunks[0]);
    let drilled = state.ui.commits.drilled.is_some();
    let help_focus = if drilled {
        state.ui.commits.diff.focus
    } else {
        state.ui.diff.focus
    };
    let pr_data = state.cache.details.get(&pr.id);
    render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, chunks[2]);
    render_help(frame, tab, drilled, help_focus, outer_chunks[1]);
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
    active: bool,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let text_w = widgets::box_text_width(width);
    // An active (cursor-focused) thread gets an accent border so it reads as
    // the selected "row"; otherwise the muted divider color.
    let border = if active { theme.accent } else { theme.divider };

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

    widgets::boxed(header, body, width, border)
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

/// Inline review threads from the loaded activity bundle, or empty.
fn activity_threads(pr_data: Option<&PrData>) -> &[ReviewThread] {
    pr_data
        .and_then(|d| match &d.activity {
            LoadState::Loaded(b) => Some(b.threads.as_slice()),
            _ => None,
        })
        .unwrap_or(&[])
}

fn render_help(frame: &mut Frame, tab: DetailTab, drilled: bool, focus: DiffFocus, area: Rect) {
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

pub(in crate::tui) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if key.code == KeyCode::Char('q') {
        return Some(Action::Quit);
    }

    let tab = match state.screen {
        Screen::Detail { tab, .. } => tab,
        _ => return None,
    };
    let drilled = state.ui.commits.drilled.is_some();
    // Focus of whichever diff view is active: the Diff tab's, or — when a
    // commit is drilled into from the Commits tab — that drill-in's.
    let diff_focus = if drilled {
        state.ui.commits.diff.focus
    } else {
        state.ui.diff.focus
    };

    // Ctrl+D / Ctrl+U: half-page scroll in whichever view is scrollable right
    // now (description, overview, commit list, or the focused diff pane).
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => return half_page_scroll(state, tab, true),
            KeyCode::Char('u') => return half_page_scroll(state, tab, false),
            _ => {}
        }
    }

    // Esc steps back one level: diff pane → tree, commit drill-in → commit
    // list, otherwise out of the detail view entirely.
    if key.code == KeyCode::Esc {
        let in_diff_pane = diff_focus == DiffFocus::Pane
            && (tab == DetailTab::Diff || (tab == DetailTab::Commits && drilled));
        if in_diff_pane {
            return Some(Action::Diff(DiffAction::FocusTree));
        }
        if tab == DetailTab::Commits && drilled {
            return Some(Action::Commits(CommitsAction::Back));
        }
        return Some(Action::Detail(DetailAction::Back));
    }

    match key.code {
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
        DetailTab::Diff => diff_nav_action(key.code, &state.ui.diff).map(Action::Diff),
        // Drilled into a commit: same diff navigation as the Diff tab, plus
        // `[`/`]` to step between commits.
        DetailTab::Commits if drilled => match key.code {
            KeyCode::Char('[') => Some(Action::Commits(CommitsAction::StepCommit(-1))),
            KeyCode::Char(']') => Some(Action::Commits(CommitsAction::StepCommit(1))),
            _ => diff_nav_action(key.code, &state.ui.commits.diff).map(Action::Diff),
        },
        // Commit list: navigate + Enter to drill into a commit's diff.
        DetailTab::Commits => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                Some(Action::Commits(CommitsAction::MoveSelection(1)))
            }
            KeyCode::Up | KeyCode::Char('k') => {
                Some(Action::Commits(CommitsAction::MoveSelection(-1)))
            }
            KeyCode::PageDown => Some(Action::Commits(CommitsAction::MoveSelection(half_page(
                state.ui.commits.viewport,
            )))),
            KeyCode::PageUp => Some(Action::Commits(CommitsAction::MoveSelection(-half_page(
                state.ui.commits.viewport,
            )))),
            KeyCode::Enter => Some(Action::Commits(CommitsAction::Open)),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
        DetailTab::Overview => match key.code {
            KeyCode::Down | KeyCode::Char('j') => Some(Action::Detail(DetailAction::OverviewScroll(1))),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Detail(DetailAction::OverviewScroll(-1))),
            KeyCode::PageDown => Some(Action::Detail(DetailAction::OverviewScroll(half_page(
                state.ui.overview_viewport,
            )))),
            KeyCode::PageUp => Some(Action::Detail(DetailAction::OverviewScroll(-half_page(
                state.ui.overview_viewport,
            )))),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
        DetailTab::Description => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                Some(Action::Detail(DetailAction::DescriptionScroll(1)))
            }
            KeyCode::Up | KeyCode::Char('k') => {
                Some(Action::Detail(DetailAction::DescriptionScroll(-1)))
            }
            KeyCode::PageDown => Some(Action::Detail(DetailAction::DescriptionScroll(half_page(
                state.ui.description_viewport,
            )))),
            KeyCode::PageUp => Some(Action::Detail(DetailAction::DescriptionScroll(-half_page(
                state.ui.description_viewport,
            )))),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
        _ => match key.code {
            KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
            _ => None,
        },
    }
}

/// The diff navigation action a key maps to, given the active diff view's
/// focus. Shared by the Diff tab and the Commits drill-in.
fn diff_nav_action(code: KeyCode, view: &DiffViewState) -> Option<DiffAction> {
    match view.focus {
        // Tree focus: navigate files, Enter jumps into the pane.
        DiffFocus::Tree => match code {
            KeyCode::Down | KeyCode::Char('j') => Some(DiffAction::MoveCursor(1)),
            KeyCode::Up | KeyCode::Char('k') => Some(DiffAction::MoveCursor(-1)),
            KeyCode::PageDown => Some(DiffAction::MoveCursor(half_page(view.tree_viewport))),
            KeyCode::PageUp => Some(DiffAction::MoveCursor(-half_page(view.tree_viewport))),
            KeyCode::Enter => Some(DiffAction::EnterPane),
            KeyCode::Char(' ') => Some(DiffAction::ToggleAtCursor),
            KeyCode::Left | KeyCode::Char('h') => Some(DiffAction::CollapseAtCursor),
            KeyCode::Right | KeyCode::Char('l') => Some(DiffAction::ExpandAtCursor),
            _ => None,
        },
        // Pane focus: scroll the diff; Enter/h/Left hand focus back to the tree.
        DiffFocus::Pane => match code {
            KeyCode::Down | KeyCode::Char('j') => Some(DiffAction::MovePaneCursor(1)),
            KeyCode::Up | KeyCode::Char('k') => Some(DiffAction::MovePaneCursor(-1)),
            KeyCode::PageDown => Some(DiffAction::MovePaneCursor(half_page(view.pane_viewport))),
            KeyCode::PageUp => Some(DiffAction::MovePaneCursor(-half_page(view.pane_viewport))),
            KeyCode::Enter | KeyCode::Left | KeyCode::Char('h') => Some(DiffAction::FocusTree),
            _ => None,
        },
    }
}

/// Ctrl+D/U half-page action for a diff view, by current focus.
fn diff_half_page(view: &DiffViewState, down: bool) -> DiffAction {
    let step = |v| if down { half_page(v) } else { -half_page(v) };
    match view.focus {
        DiffFocus::Pane => DiffAction::MovePaneCursor(step(view.pane_viewport)),
        DiffFocus::Tree => DiffAction::MoveCursor(step(view.tree_viewport)),
    }
}

/// The scroll action Ctrl+D/U should fire, picked from whichever view is
/// currently scrollable. `None` on non-scrolling contexts (e.g. Builds).
fn half_page_scroll(state: &AppState, tab: DetailTab, down: bool) -> Option<Action> {
    let step = |viewport| {
        let h = half_page(viewport);
        if down { h } else { -h }
    };
    match tab {
        DetailTab::Description => Some(Action::Detail(DetailAction::DescriptionScroll(step(
            state.ui.description_viewport,
        )))),
        DetailTab::Overview => Some(Action::Detail(DetailAction::OverviewScroll(step(
            state.ui.overview_viewport,
        )))),
        DetailTab::Diff => Some(Action::Diff(diff_half_page(&state.ui.diff, down))),
        DetailTab::Commits if state.ui.commits.drilled.is_some() => {
            Some(Action::Diff(diff_half_page(&state.ui.commits.diff, down)))
        }
        DetailTab::Commits => Some(Action::Commits(CommitsAction::MoveSelection(step(
            state.ui.commits.viewport,
        )))),
        DetailTab::Builds => None,
    }
}
