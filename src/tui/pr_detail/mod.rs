pub mod checks;
pub mod commits;
pub mod description;
pub mod diff;
pub mod file_tree;
pub mod overview;

use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::{
    app::state::{AppState, LoadState, PrData, Screen, UiMemory},
    domain::pr::PullRequest,
    tui::{Action, theme},
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

pub fn render(frame: &mut Frame, state: &mut AppState, pr_id: u64, tab: DetailTab, area: Rect) {
    let prs = match &state.cache.prs {
        LoadState::Loaded(prs) => prs,
        _ => return,
    };
    let Some(pr) = prs.iter().find(|p| p.id == pr_id) else {
        return;
    };

    // Outer yellow frame wraps the page but the help text lives below it —
    // same chrome split as pr_list, so the footer sits outside the border.
    let theme = theme::current();
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(outer_chunks[0]);
    frame.render_widget(outer, outer_chunks[0]);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header (title + blank + meta)
            Constraint::Length(1), // spacer between header and tabs
            Constraint::Min(0),    // tabs + divider + content
        ])
        .split(inner);

    render_header(frame, pr, chunks[0]);
    let pr_data = state.cache.details.get(&pr.id);
    render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, chunks[2]);
    render_help(frame, outer_chunks[1]);
}

/// Render a PR description (markdown) into ratatui lines via charmed-glamour,
/// already wrapped to `width`. Glamour emits ANSI-styled text which we bridge
/// into ratatui via `ansi-to-tui`. Glamour uses CommonMark/GFM, so `-`/`+`/`*`
/// bullets, numbered lists and checkboxes all work without shims.
///
/// `glamour::render` returns a `String` rather than a `Result`, so a panic on
/// some pathological body would otherwise take down the whole TUI. We isolate
/// it behind `catch_unwind` and fall back to the raw body, so rendering can
/// never crash the app — defence-in-depth around a young dependency.
pub(super) fn render_markdown(body: &str, width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::default()];
    }
    let rendered = std::panic::catch_unwind(|| {
        let ansi = glamour::Renderer::new()
            .with_style(glamour::Style::Dark)
            .with_word_wrap(width as usize)
            .render(body);
        ansi.into_text().map(|text| text.lines)
    });
    let lines = match rendered {
        Ok(Ok(lines)) => lines,
        // ANSI bridge failed, or glamour panicked: fall back to the raw body.
        _ => body.lines().map(|l| Line::raw(l.to_string())).collect(),
    };
    if lines.is_empty() {
        vec![Line::default()]
    } else {
        lines
    }
}

pub(super) fn description_body(pr: &PullRequest) -> &str {
    pr.description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("(ingen beskrivning)")
}

/// glamour's Dark theme wraps the document with blank-line margins. For inline
/// content (comments, replies) where vertical space is at a premium we strip
/// those leading and trailing blanks so the body sits flush against whatever
/// frames it.
pub(super) fn trim_blank_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    while lines.first().is_some_and(is_blank_line) {
        lines.remove(0);
    }
    while lines.last().is_some_and(is_blank_line) {
        lines.pop();
    }
    lines
}

fn is_blank_line(line: &Line<'static>) -> bool {
    line.spans.is_empty() || line.spans.iter().all(|s| s.content.trim().is_empty())
}

/// A simple thumb-style scrollbar drawn in the rightmost column of `area`.
///
/// ratatui's built-in `Scrollbar` is mathematically correct but the thumb is
/// sized proportionally to the visible content. For short scroll ranges that
/// means the thumb is so large its *top* only inches down even at max scroll,
/// which doesn't feel like "scrolled to the bottom" to most users.
///
/// We instead use a fixed 3-row thumb that slides from `0` to `track_len - 1`.
/// The visible thumb compresses to 1-2 rows at the very bottom (the lower
/// rows fall outside the track), which is a small visual cost for clear
/// "I'm at the bottom" feedback.
pub(super) fn render_thumb_scrollbar(frame: &mut Frame, scroll: u16, max_scroll: u16, area: Rect) {
    if max_scroll == 0 || area.height < 1 || area.width < 1 {
        return;
    }
    let theme = theme::current();
    let track_len = area.height as usize;
    let thumb_size = 3usize.min(track_len);
    let max_thumb_top = track_len.saturating_sub(1);
    let thumb_top = (scroll as usize * max_thumb_top) / max_scroll as usize;

    let lines: Vec<Line<'static>> = (0..track_len)
        .map(|y| {
            if y >= thumb_top && y < thumb_top + thumb_size {
                Line::styled("█", Style::default().fg(theme.accent))
            } else {
                Line::styled("│", Style::default().fg(theme.muted))
            }
        })
        .collect();

    let bar_area = Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    };
    frame.render_widget(Paragraph::new(lines), bar_area);
}

/// Render a review thread as a `┃`-bar-prefixed block. Shared between the
/// inline diff view and the Overview tab so review comments look identical in
/// both places.
pub(super) fn render_inline_thread(
    thread: &crate::domain::comment::ReviewThread,
    width: u16,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let body_width = width.saturating_sub(1);
    let cyan = Style::default().fg(theme::current().info);

    for (i, comment) in thread.comments.iter().enumerate() {
        let age = relative_age(comment.created, now);
        let header_text = if i == 0 {
            format!("┃ @{} · {}", comment.author.username, age)
        } else {
            format!("┃ ↳ @{} · {}", comment.author.username, age)
        };
        out.push(Line::styled(header_text, cyan));

        let body_lines = trim_blank_lines(render_markdown(&comment.content, body_width));
        for bline in body_lines {
            let mut spans = vec![Span::styled("┃", cyan)];
            spans.extend(bline.spans);
            out.push(Line::from(spans));
        }

        if i + 1 < thread.comments.len() {
            out.push(Line::styled("┃", cyan));
        }
    }

    out
}

pub(super) fn relative_age(
    when: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let delta = now - when;
    let days = delta.num_days();
    if days >= 1 {
        if days == 1 {
            "1d ago".to_string()
        } else {
            format!("{}d ago", days)
        }
    } else {
        let hours = delta.num_hours();
        if hours >= 1 {
            if hours == 1 {
                "1h ago".to_string()
            } else {
                format!("{}h ago", hours)
            }
        } else {
            "just now".to_string()
        }
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
        tab_spans.push(Span::styled(
            format!("{}  {}", t.icon(), t.label()),
            style,
        ));
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
        crate::domain::pr::PrStatus::Draft => theme.status_draft,
        crate::domain::pr::PrStatus::Open => theme.status_open,
        crate::domain::pr::PrStatus::Merged => theme.status_merged,
        crate::domain::pr::PrStatus::Declined => theme.status_declined,
    };

    let title_line = Line::from(vec![
        Span::styled(format!("#{} ", pr.id), Style::default().fg(theme.muted)),
        Span::styled(
            pr.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);

    // Powerline rounded half-circles (Nerd Font U+E0B6 / U+E0B4) flank the
    // padded label to give the badge pill-shaped rounded ends. The edges have
    // fg=status_color with no bg, so the rounded curve bleeds into whatever
    // sits behind the header.
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
        Span::styled(pr.source_branch.clone(), Style::default().fg(theme.success)),
        Span::raw(" → "),
        Span::styled(pr.target_branch.clone(), Style::default().fg(theme.accent)),
    ];
    let right_spans = build_reviewer_spans(pr);

    // 2-col padding on each side (matches `Padding::horizontal(2)` on the
    // block below) so the right segment ends 2 cols before the area edge.
    let content_width = (area.width as usize).saturating_sub(4);
    let left_visible: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let right_visible: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let gap = content_width
        .saturating_sub(left_visible + right_visible)
        .max(1);

    let mut meta_spans = left_spans;
    if !right_spans.is_empty() {
        meta_spans.push(Span::raw(" ".repeat(gap)));
        meta_spans.extend(right_spans);
    }
    let meta_line = Line::from(meta_spans);

    let paragraph = Paragraph::new(vec![title_line, Line::default(), meta_line])
        .block(Block::default().padding(Padding::horizontal(2)));
    frame.render_widget(paragraph, area);
}

fn build_reviewer_spans(pr: &PullRequest) -> Vec<Span<'static>> {
    use crate::domain::review::ReviewerState;
    let theme = theme::current();
    if pr.reviewers.is_empty() {
        return Vec::new();
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, reviewer) in pr.reviewers.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let (icon, color) = match reviewer.state {
            ReviewerState::Approved => ("\u{f058}", theme.success), //  check-circle
            ReviewerState::ChangesRequested => ("\u{f057}", theme.error), //  times-circle
            ReviewerState::Commented => ("\u{f075}", theme.info),   //  comment
        };
        spans.push(Span::styled(
            format!("@{}", reviewer.author.username),
            Style::default().fg(theme.info),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(icon.to_string(), Style::default().fg(color)));
    }
    spans
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
        DetailTab::Overview => overview::render(frame, pr_data, ui, inset),
        DetailTab::Diff => diff::render(frame, pr, pr_data, &ui.diff, inset),
        DetailTab::Commits => commits::render(frame, pr_data, inset),
        DetailTab::Builds => checks::render(frame, inset),
    }
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  1-5 / h/l: tab  j/k: scroll  esc: back  q: quit")
        .style(Style::default().fg(theme::current().muted));
    frame.render_widget(help, area);
}

pub(super) fn render_placeholder(frame: &mut Frame, text: &str, area: Rect) {
    let paragraph = Paragraph::new(text).style(Style::default().fg(theme::current().muted));
    frame.render_widget(paragraph, area);
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    if matches!(key, KeyCode::Char('q')) {
        return Some(Action::Quit);
    }

    match key {
        KeyCode::Esc => return Some(Action::Back),
        KeyCode::Tab => return Some(Action::NextTab),
        KeyCode::BackTab => return Some(Action::PrevTab),
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            if let Some(&t) = DetailTab::ALL.get(idx) {
                return Some(Action::SelectTab(t));
            }
        }
        _ => {}
    }

    let tab = match state.screen {
        Screen::Detail { tab, .. } => tab,
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
        DetailTab::Overview => match key {
            KeyCode::Down | KeyCode::Char('j') => Some(Action::OverviewScrollDown),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::OverviewScrollUp),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::NextTab),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::PrevTab),
            _ => None,
        },
        DetailTab::Description => match key {
            KeyCode::Down | KeyCode::Char('j') => Some(Action::DescriptionScrollDown),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::DescriptionScrollUp),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::NextTab),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::PrevTab),
            _ => None,
        },
        _ => match key {
            KeyCode::Right | KeyCode::Char('l') => Some(Action::NextTab),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::PrevTab),
            _ => None,
        },
    }
}
