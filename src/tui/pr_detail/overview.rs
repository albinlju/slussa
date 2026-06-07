use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::state::{LoadState, PrData, UiMemory},
    domain::comment::{Comment, ReviewThread},
    tui::{
        pr_detail::{
            relative_age, render_markdown, render_thumb_scrollbar, trim_blank_lines,
        },
        spinner_frame, theme,
    },
};

/// Width of the left timeline column (`● ` or `│ ` — glyph + a trailing
/// space before content).
const TIMELINE_COL: u16 = 2;

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, ui: &mut UiMemory, area: Rect) {
    let theme = theme::current();
    let comments_state = pr_data.map(|d| &d.comments);
    let threads_state = pr_data.map(|d| &d.review_threads);

    let comments_ready = matches!(comments_state, Some(LoadState::Loaded(_)));
    let threads_ready = matches!(threads_state, Some(LoadState::Loaded(_)));

    // Wait until both fetches resolve before deciding "no comments". Otherwise
    // we flash "(no comments)" the moment one source finishes empty before the
    // other lands with content.
    if !comments_ready || !threads_ready {
        let p = Paragraph::new(format!("{}  Loading...", spinner_frame()))
            .style(Style::default().fg(theme.warning));
        frame.render_widget(p, area);
        return;
    }

    let comments: &[Comment] = match comments_state {
        Some(LoadState::Loaded(c)) => c,
        _ => &[],
    };
    let threads: &[ReviewThread] = match threads_state {
        Some(LoadState::Loaded(t)) => t,
        _ => &[],
    };

    if comments.is_empty() && threads.is_empty() {
        let p = Paragraph::new("(no comments)").style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    // Reserve rightmost column for the scrollbar so wrapped markdown doesn't
    // get clipped or overlap the thumb.
    let content_width = area.width.saturating_sub(1);
    let lines = build_overview_lines(comments, threads, content_width.saturating_sub(TIMELINE_COL));

    let total = lines.len();
    let visible = area.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;
    let scroll = ui.overview_scroll.min(max_scroll);
    ui.overview_scroll = scroll;

    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: content_width,
        height: area.height,
    };
    let p = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(p, content_area);

    if max_scroll > 0 {
        render_thumb_scrollbar(frame, scroll, max_scroll, area);
    }
}

enum Event<'a> {
    Issue(&'a Comment),
    Review(&'a ReviewThread),
}

impl<'a> Event<'a> {
    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Event::Issue(c) => c.created,
            Event::Review(t) => t
                .comments
                .first()
                .map(|c| c.created)
                .unwrap_or_else(Utc::now),
        }
    }
}

fn build_overview_lines(
    comments: &[Comment],
    threads: &[ReviewThread],
    width: u16,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let mut events: Vec<Event<'_>> = Vec::with_capacity(comments.len() + threads.len());
    events.extend(comments.iter().map(Event::Issue));
    events.extend(threads.iter().map(Event::Review));
    events.sort_by_key(|e| e.timestamp());

    let now = Utc::now();

    // Render each event's content lines first (no left column), then assemble
    // with the timeline column prepended — circle on the first line of each
    // event, vertical connector on all other lines and on the gap rows
    // between events.
    let mut blocks: Vec<(EventStyle, Vec<Line<'static>>)> = Vec::with_capacity(events.len());
    for event in &events {
        match event {
            Event::Issue(c) => blocks.push((EventStyle::Comment, build_issue_lines(c, width, now))),
            Event::Review(t) => {
                if let Some(lines) = build_review_lines(t, width, now) {
                    blocks.push((EventStyle::Review, lines));
                }
            }
        }
    }

    let connector_style = Style::default().fg(theme.divider);
    let connector = Span::styled("│ ".to_string(), connector_style);

    let mut all: Vec<Line<'static>> = Vec::new();
    for (i, (style, lines)) in blocks.into_iter().enumerate() {
        if i > 0 {
            all.push(Line::from(connector.clone()));
            all.push(Line::from(connector.clone()));
        }
        let circle = Span::styled(
            "● ".to_string(),
            Style::default()
                .fg(style.color(&theme))
                .add_modifier(Modifier::BOLD),
        );
        for (j, line) in lines.into_iter().enumerate() {
            let marker = if j == 0 { circle.clone() } else { connector.clone() };
            let mut spans = vec![marker];
            spans.extend(line.spans);
            all.push(Line::from(spans));
        }
    }

    if all.is_empty() {
        all.push(Line::default());
    }
    all
}

#[derive(Clone, Copy)]
enum EventStyle {
    /// Plain issue/discussion comment.
    Comment,
    /// Inline review comment anchored to a diff line.
    Review,
}

impl EventStyle {
    fn color(self, theme: &crate::tui::theme::Theme) -> ratatui::style::Color {
        match self {
            Self::Comment => theme.info,
            Self::Review => theme.accent,
        }
    }
}

fn build_issue_lines(c: &Comment, width: u16, now: DateTime<Utc>) -> Vec<Line<'static>> {
    let text_width = box_text_width(width);
    let header = issue_comment_header(&c.author.username, c.created, now);
    // Glamour's Dark theme adds a 2-col document margin to every rendered
    // line. Stripping it pulls the comment text flush against the box's
    // inner padding instead of sitting another two cols in.
    let body = trim_blank_lines(strip_glamour_margin(
        render_markdown(&c.content, text_width + 2),
        2,
    ));
    boxed(header, body, width)
}

fn build_review_lines(
    thread: &ReviewThread,
    width: u16,
    now: DateTime<Utc>,
) -> Option<Vec<Line<'static>>> {
    let theme = theme::current();
    if thread.comments.is_empty() {
        return None;
    }
    let text_width = box_text_width(width);
    let first = thread.comments.first()?;
    let header = issue_comment_header(&first.author.username, first.created, now);

    let mut body: Vec<Line<'static>> = Vec::new();

    // Anchor line: file path + optional line number + resolved badge.
    let location = match thread.line {
        Some(l) => format!("{}:{}", thread.path, l),
        None => thread.path.clone(),
    };
    let mut anchor_spans = vec![Span::styled(location, Style::default().fg(theme.accent))];
    if thread.resolved {
        anchor_spans.push(Span::styled(
            " · resolved",
            Style::default().fg(theme.success),
        ));
    }
    body.push(Line::from(anchor_spans));
    body.push(Line::raw(""));

    if !thread.diff_hunk.is_empty() {
        body.extend(styled_diff_hunk(&thread.diff_hunk, text_width));
        body.push(Line::raw(""));
    }

    // First comment body + replies. The first author is already in the box
    // header so we skip the per-comment header for them; replies still get
    // a `↳ @user` line.
    for (i, comment) in thread.comments.iter().enumerate() {
        if i > 0 {
            body.push(Line::raw(""));
            let age = relative_age(comment.created, now);
            body.push(Line::from(vec![
                Span::styled(
                    format!("↳ @{}", comment.author.username),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" · {}", age), Style::default().fg(theme.muted)),
            ]));
        }
        body.extend(trim_blank_lines(strip_glamour_margin(
            render_markdown(&comment.content, text_width + 2),
            2,
        )));
    }

    Some(boxed(header, body, width))
}

/// Text width available inside the box — outer minus 2 border cols and
/// 2 cols of internal horizontal padding.
fn box_text_width(outer: u16) -> u16 {
    outer.saturating_sub(4)
}

/// Walk each line's spans and strip up to `n` leading whitespace cells from
/// the start of the line. Used to peel off glamour's fixed document margin
/// so wrapped markdown sits flush against the box padding.
fn strip_glamour_margin(lines: Vec<Line<'static>>, n: usize) -> Vec<Line<'static>> {
    lines.into_iter().map(|line| strip_line_left(line, n)).collect()
}

fn strip_line_left(line: Line<'static>, mut budget: usize) -> Line<'static> {
    let line_style = line.style;
    let mut out: Vec<Span<'static>> = Vec::with_capacity(line.spans.len());
    let mut done = false;
    for span in line.spans {
        if done || budget == 0 {
            out.push(span);
            continue;
        }
        let chars: Vec<char> = span.content.chars().collect();
        let strip_count = chars
            .iter()
            .take(budget)
            .take_while(|c| c.is_whitespace())
            .count();
        budget -= strip_count;
        if strip_count < chars.len() {
            let remainder: String = chars.into_iter().skip(strip_count).collect();
            out.push(Span::styled(remainder, span.style));
            done = true;
        }
        // else: whole span was whitespace within the budget — drop it.
    }
    Line::from(out).style(line_style)
}

/// Wrap a header line + body lines in a `┌──┐ │ ├──┤ │ └──┘` frame at
/// the given outer `width`. The first row inside is the header, separated
/// from the body by a `├─┤` divider.
fn boxed(header: Line<'static>, body: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let theme = theme::current();
    let style = Style::default().fg(theme.divider);
    let inner = (width as usize).saturating_sub(2); // between left/right border
    let text_w = inner.saturating_sub(2); // also minus 1-col padding on each side

    let bar = |s: String| Line::from(Span::styled(s, style));

    let mut out: Vec<Line<'static>> = Vec::new();
    out.push(bar(format!("┌{}┐", "─".repeat(inner))));
    out.push(wrap_box_line(header, text_w, style));
    out.push(bar(format!("├{}┤", "─".repeat(inner))));
    for line in body {
        out.push(wrap_box_line(line, text_w, style));
    }
    out.push(bar(format!("└{}┘", "─".repeat(inner))));
    out
}

fn wrap_box_line(line: Line<'static>, text_w: usize, border: Style) -> Line<'static> {
    let visible: usize = line.spans.iter().map(|s| s.width()).sum();
    let pad = text_w.saturating_sub(visible);
    let line_style = line.style;
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 3);
    spans.push(Span::styled("│ ".to_string(), border));
    for s in line.spans {
        let merged = line_style.patch(s.style);
        spans.push(Span::styled(s.content, merged));
    }
    spans.push(Span::raw(" ".repeat(pad + 1)));
    spans.push(Span::styled("│".to_string(), border));
    Line::from(spans)
}

fn issue_comment_header(
    username: &str,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Line<'static> {
    let theme = theme::current();
    Line::from(vec![
        Span::styled(
            username.to_string(),
            Style::default()
                .fg(theme.info)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" commented", Style::default().fg(theme.muted)),
        Span::styled(
            format!(" · {}", relative_age(created, now)),
            Style::default().fg(theme.muted),
        ),
    ])
}

/// Diff hunk styled to match the Diff tab — full-row bg tint on added/removed
/// lines, strong-color prefix for `+`/`-`, plain muted for context. `width` is
/// the row width inside the box bracket so the bg fill reaches the right edge.
fn styled_diff_hunk(hunk: &str, width: u16) -> Vec<Line<'static>> {
    let theme = theme::current();
    let row_w = width as usize;
    hunk.lines()
        .map(|line| {
            if line.starts_with("@@") {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme.info),
                ))
            } else if let Some(content) = line.strip_prefix('+') {
                let visible = 1 + content.chars().count();
                let pad = row_w.saturating_sub(visible);
                let bg = theme.diff_added_bg;
                Line::from(vec![
                    Span::styled(
                        "+".to_string(),
                        Style::default()
                            .fg(theme.diff_added)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}{}", content, " ".repeat(pad)),
                        Style::default().fg(theme.fg).bg(bg),
                    ),
                ])
            } else if let Some(content) = line.strip_prefix('-') {
                let visible = 1 + content.chars().count();
                let pad = row_w.saturating_sub(visible);
                let bg = theme.diff_removed_bg;
                Line::from(vec![
                    Span::styled(
                        "-".to_string(),
                        Style::default()
                            .fg(theme.diff_removed)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}{}", content, " ".repeat(pad)),
                        Style::default().fg(theme.muted).bg(bg),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme.diff_context),
                ))
            }
        })
        .collect()
}
