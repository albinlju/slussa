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
            relative_age, render_inline_thread, render_markdown, render_thumb_scrollbar,
            trim_blank_lines,
        },
        spinner_frame, theme,
    },
};

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
    let lines = build_overview_lines(comments, threads, content_width);

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

    let mut all: Vec<Line<'static>> = Vec::new();
    let now = Utc::now();
    let separator_width = width as usize;

    for (i, event) in events.iter().enumerate() {
        if i > 0 {
            all.push(Line::raw(""));
            all.push(Line::styled(
                "─".repeat(separator_width),
                Style::default().fg(theme.muted),
            ));
            all.push(Line::raw(""));
        }

        match event {
            Event::Issue(c) => extend_issue_comment(&mut all, c, width, now),
            Event::Review(t) => extend_review_thread(&mut all, t, width, now),
        }
    }

    if all.is_empty() {
        all.push(Line::default());
    }
    all
}

fn extend_issue_comment(
    out: &mut Vec<Line<'static>>,
    c: &Comment,
    width: u16,
    now: DateTime<Utc>,
) {
    out.push(issue_comment_header(&c.author.username, c.created, now));
    out.push(Line::raw(""));
    out.extend(trim_blank_lines(render_markdown(&c.content, width)));
}

fn extend_review_thread(
    out: &mut Vec<Line<'static>>,
    thread: &ReviewThread,
    width: u16,
    now: DateTime<Utc>,
) {
    let theme = theme::current();
    if thread.comments.is_empty() {
        return;
    }

    // Anchor line: file path + optional line number + resolved badge. The
    // @user attribution lives inside the bar-styled thread below to keep this
    // visually consistent with the inline Diff-tab rendering.
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
    out.push(Line::from(anchor_spans));
    out.push(Line::raw(""));

    if !thread.diff_hunk.is_empty() {
        out.extend(diff_hunk_lines(&thread.diff_hunk));
    }

    out.extend(render_inline_thread(thread, width, now));
}

fn issue_comment_header(
    username: &str,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Line<'static> {
    let theme = theme::current();
    Line::from(vec![
        Span::styled(
            format!("@{}", username),
            Style::default()
                .fg(theme.info)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {}", relative_age(created, now)),
            Style::default().fg(theme.muted),
        ),
    ])
}

fn diff_hunk_lines(hunk: &str) -> Vec<Line<'static>> {
    let theme = theme::current();
    hunk.lines()
        .map(|line| {
            let style = if line.starts_with("@@") {
                Style::default().fg(theme.info)
            } else if line.starts_with('+') {
                Style::default().fg(theme.diff_added)
            } else if line.starts_with('-') {
                Style::default().fg(theme.diff_removed)
            } else {
                Style::default().fg(theme.muted)
            };
            Line::styled(line.to_string(), style)
        })
        .collect()
}
