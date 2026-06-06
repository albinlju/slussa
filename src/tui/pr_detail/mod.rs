pub mod checks;
pub mod commits;
pub mod diff;
pub mod file_tree;
pub mod overview;

use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::state::{AppState, LoadState, PrData, Screen, UiMemory},
    domain::pr::PullRequest,
    tui::Action,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Overview,
    Diff,
    Commits,
    Builds,
}

impl DetailTab {
    pub const ALL: [Self; 4] = [Self::Overview, Self::Diff, Self::Commits, Self::Builds];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Diff => "Diff",
            Self::Commits => "Commits",
            Self::Builds => "Builds",
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

    if state.ui.description_expanded {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);
        render_header(frame, pr, chunks[0]);
        render_description_expanded(frame, pr, &mut state.ui, chunks[1]);
        render_help_expanded(frame, chunks[2]);
    } else {
        // Parse markdown once per frame and reuse the result for both the layout
        // calculation and the actual rendering (inner width = full width - borders).
        let lines = trim_blank_lines(render_markdown(
            description_body(pr),
            area.width.saturating_sub(2),
        ));
        let (desc_height, truncated) = desc_layout(lines.len());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(desc_height),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);
        render_header(frame, pr, chunks[0]);
        render_description(frame, lines, truncated, chunks[1]);
        let pr_data = state.cache.details.get(&pr.id);
        render_tabs_and_content(frame, pr, pr_data, &mut state.ui, tab, chunks[2]);
        render_help(frame, chunks[3]);
    }
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

fn description_body(pr: &PullRequest) -> &str {
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
    let track_len = area.height as usize;
    let thumb_size = 3usize.min(track_len);
    let max_thumb_top = track_len.saturating_sub(1);
    let thumb_top = (scroll as usize * max_thumb_top) / max_scroll as usize;

    let lines: Vec<Line<'static>> = (0..track_len)
        .map(|y| {
            if y >= thumb_top && y < thumb_top + thumb_size {
                Line::styled("█", Style::default().fg(Color::Yellow))
            } else {
                Line::styled("│", Style::default().fg(Color::DarkGray))
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
    let cyan = Style::default().fg(Color::Cyan);

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

fn render_description(frame: &mut Frame, lines: Vec<Line<'static>>, truncated: bool, area: Rect) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(" Description ");

    if truncated {
        block = block.title_bottom(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "⇣ more",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" · d to expand ", Style::default().fg(Color::DarkGray)),
            ])
            .right_aligned(),
        );
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_description_expanded(frame: &mut Frame, pr: &PullRequest, ui: &mut UiMemory, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Description ")
        .title_bottom(
            Line::from(" j/k scroll · d/esc collapse ")
                .right_aligned()
                .style(Style::default().fg(Color::DarkGray)),
        );

    let inner = block.inner(area);
    let lines = trim_blank_lines(render_markdown(description_body(pr), inner.width));
    let total_wrapped = lines.len();
    let visible = inner.height as usize;
    let max_scroll = total_wrapped.saturating_sub(visible) as u16;
    let scroll = ui.description_scroll.min(max_scroll);
    ui.description_scroll = scroll;

    let paragraph = Paragraph::new(lines).scroll((scroll, 0)).block(block);
    frame.render_widget(paragraph, area);

    if max_scroll > 0 {
        let sb_area = area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        });
        render_thumb_scrollbar(frame, scroll, max_scroll, sb_area);
    }
}

const DESC_MAX_HEIGHT: u16 = 8;

fn desc_layout(total_lines: usize) -> (u16, bool) {
    let total_lines = total_lines.max(1);
    let max_content_rows = (DESC_MAX_HEIGHT as usize).saturating_sub(2);
    let truncated = total_lines > max_content_rows;
    let content_rows = total_lines.min(max_content_rows);
    let height = ((content_rows + 2).max(3)) as u16;
    (height, truncated)
}

fn render_tabs_and_content(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
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

    let content_block = Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM);
    let content_inner = content_block.inner(chunks[1]);
    frame.render_widget(content_block, chunks[1]);

    render_content(frame, pr, pr_data, ui, tab, content_inner);
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
            format!(" {} ", pr.status.label()),
            Style::default().fg(Color::White).bg(Color::Green),
        ),
        Span::styled(
            format!(" @{}", pr.author.username),
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
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    tab: DetailTab,
    area: Rect,
) {
    match tab {
        DetailTab::Overview => overview::render(frame, pr_data, ui, area),
        DetailTab::Diff => diff::render(frame, pr, pr_data, &ui.diff, area),
        DetailTab::Commits => commits::render(frame, pr_data, area),
        DetailTab::Builds => checks::render(frame, area),
    }
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  1-4 / h/l: tab  d: description  esc: back  q: quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

fn render_help_expanded(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  j/k: scroll  d / esc: collapse  q: quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

pub(super) fn render_placeholder(frame: &mut Frame, text: &str, area: Rect) {
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    if matches!(key, KeyCode::Char('q')) {
        return Some(Action::Quit);
    }

    if state.ui.description_expanded {
        return match key {
            KeyCode::Esc | KeyCode::Char('d') => Some(Action::ToggleDescription),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::DescriptionScrollDown),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::DescriptionScrollUp),
            _ => None,
        };
    }

    match key {
        KeyCode::Char('d') => return Some(Action::ToggleDescription),
        KeyCode::Esc => return Some(Action::Back),
        KeyCode::Tab => return Some(Action::NextTab),
        KeyCode::BackTab => return Some(Action::PrevTab),
        KeyCode::Char(c @ '1'..='4') => {
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
        _ => match key {
            KeyCode::Right | KeyCode::Char('l') => Some(Action::NextTab),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::PrevTab),
            _ => None,
        },
    }
}
