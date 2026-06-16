use std::cmp::Reverse;

use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use crate::{
    app::state::{LoadState, PrData, UiMemory},
    domain::{
        comment::{Comment, ReviewThread},
        diff::Diff,
        event::{EventKind, TimelineEvent},
        pr::PullRequest,
        review::ReviewerState,
    },
    tui::{format, icons, layout, theme, widgets},
};

const TIMELINE_RIGHT_PAD: u16 = 3;

const SIDEBAR_WIDTH: u16 = 30;
const SIDEBAR_BREAKPOINT: u16 = 64;

pub fn render(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    area: Rect,
) {
    let (body_area, sidebar_area, scrollbar_area) = if area.width >= SIDEBAR_BREAKPOINT {
        let [body, sidebar, scrollbar] = layout::split(
            area,
            Direction::Horizontal,
            [
                Constraint::Min(0),
                Constraint::Length(SIDEBAR_WIDTH),
                Constraint::Length(1),
            ],
        );
        (body, Some(sidebar), scrollbar)
    } else {
        let [body, scrollbar] = layout::split(
            area,
            Direction::Horizontal,
            [Constraint::Min(0), Constraint::Length(1)],
        );
        (body, None, scrollbar)
    };

    if let Some(sidebar) = sidebar_area {
        render_sidebar(frame, pr, pr_data, sidebar);
    }
    render_timeline(frame, pr_data, ui, &pr.author.username, body_area, scrollbar_area);
}

fn render_sidebar(frame: &mut Frame, pr: &PullRequest, pr_data: Option<&PrData>, area: Rect) {
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(theme.divider))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let now = Utc::now();
    let mut lines: Vec<Line<'static>> = Vec::new();

    section_heading(&mut lines, "Reviewers");
    lines.extend(reviewers(pr));
    lines.push(Line::default());

    section_heading(&mut lines, "Builds");
    lines.extend(builds_summary(pr_data));
    lines.push(Line::default());

    if !pr.labels.is_empty() {
        section_heading(&mut lines, "Labels");
        lines.extend(labels(pr));
        lines.push(Line::default());
    }

    section_heading(&mut lines, "Details");
    lines.extend(details(pr, now));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn section_heading(lines: &mut Vec<Line<'static>>, title: &str) {
    let theme = theme::current();
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )));
}

fn reviewers(pr: &PullRequest) -> Vec<Line<'static>> {
    let theme = theme::current();
    if pr.reviewers.is_empty() {
        return vec![muted_line("—")];
    }
    pr.reviewers
        .iter()
        .map(|r| {
            let (icon, color) = match r.state {
                ReviewerState::Approved => (icons::CHECK_CIRCLE, theme.success),
                ReviewerState::ChangesRequested => (icons::TIMES_CIRCLE, theme.error),
                ReviewerState::Commented => (icons::CIRCLE_O, theme.muted),
            };
            Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(
                    format!("@{}", r.author.username),
                    Style::default().fg(theme.info),
                ),
            ])
        })
        .collect()
}

fn builds_summary(pr_data: Option<&PrData>) -> Vec<Line<'static>> {
    match pr_data.map(|d| &d.builds) {
        Some(LoadState::Loaded(builds)) if !builds.is_empty() => {
            let stats = super::super::build_status::build_stats(builds);
            vec![
                Line::from(Span::styled(
                    format!("{}/{} passing", stats.passing, stats.total),
                    Style::default()
                        .fg(stats.accent())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(super::super::build_status::progress_bar(builds)),
            ]
        }
        Some(LoadState::Loaded(_)) => vec![muted_line("no builds")],
        Some(LoadState::Failed(_)) => vec![muted_line("unavailable")],
        _ => vec![widgets::loading("loading…")],
    }
}

fn labels(pr: &PullRequest) -> Vec<Line<'static>> {
    let accent = Style::default().fg(theme::current().accent);
    pr.labels
        .iter()
        .map(|label| Line::from(Span::styled(label.clone(), accent)))
        .collect()
}

fn details(pr: &PullRequest, now: DateTime<Utc>) -> Vec<Line<'static>> {
    let theme = theme::current();
    let fg = Style::default().fg(theme.fg);
    let detail = |key: &str, value: Vec<Span<'static>>| -> Line<'static> {
        let mut spans = vec![Span::styled(
            format!("{key:<8}"),
            Style::default().fg(theme.muted),
        )];
        spans.extend(value);
        Line::from(spans)
    };
    vec![
        detail(
            "opened",
            vec![Span::styled(format::relative_age(pr.created, now), fg)],
        ),
        detail(
            "updated",
            vec![Span::styled(format::relative_age(pr.updated, now), fg)],
        ),
        detail(
            "diff",
            vec![
                Span::styled(
                    format!("+{}", pr.additions),
                    Style::default().fg(theme.diff_added),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("-{}", pr.deletions),
                    Style::default().fg(theme.diff_removed),
                ),
            ],
        ),
        detail(
            "files",
            vec![Span::styled(pr.changed_files.to_string(), fg)],
        ),
    ]
}

fn muted_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(theme::current().muted),
    ))
}

fn render_timeline(
    frame: &mut Frame,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    author: &str,
    area: Rect,
    scrollbar_area: Rect,
) {
    let Some(activity) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.activity), "activity", area)
    else {
        return;
    };

    if activity.comments.is_empty() && activity.threads.is_empty() && activity.events.is_empty() {
        frame.render_widget(widgets::empty_state("(no activity)"), area);
        return;
    }

    let diff = pr_data.and_then(|d| match &d.diff {
        LoadState::Loaded(diff) => Some(diff),
        _ => None,
    });

    let count = timeline_count(&activity.comments, &activity.threads, &activity.events);
    let cursor = ui.overview_cursor.min(count.saturating_sub(1));
    ui.overview_item_count = count;
    ui.overview_cursor = cursor;

    let blocks = build_blocks(
        &activity.comments,
        &activity.threads,
        &activity.events,
        diff,
        area.width.saturating_sub(TIMELINE_RIGHT_PAD),
        cursor,
        author,
    );

    let (content, navs) = timeline_rail(blocks);
    ui.overview_reply = navs.get(cursor).and_then(|n| n.reply_to);

    let viewport = area.height as usize;
    let max_scroll = content.len().saturating_sub(viewport) as u16;
    let scroll = navs.get(cursor).map_or(ui.overview_scroll, |n| {
        scroll_to_item(ui.overview_scroll, n.start, n.span, content.len(), viewport)
    });
    ui.overview_scroll = scroll.min(max_scroll);
    ui.overview_viewport = area.height;

    let content_area = Rect {
        width: area.width.saturating_sub(TIMELINE_RIGHT_PAD),
        ..area
    };
    frame.render_widget(
        Paragraph::new(content).scroll((ui.overview_scroll, 0)),
        content_area,
    );

    if max_scroll > 0 {
        let bar = widgets::scrollbar(ui.overview_scroll, max_scroll, scrollbar_area.height);
        frame.render_widget(Paragraph::new(bar), scrollbar_area);
    }
}

fn scroll_to_item(scroll: u16, start: usize, span: usize, total: usize, viewport: usize) -> u16 {
    let max_scroll = total.saturating_sub(viewport) as u16;
    let start = start as u16;
    let end = start + (span.max(1) as u16) - 1;
    let mut s = scroll.min(max_scroll);
    if start < s {
        s = start;
    } else if viewport > 0 && end >= s + viewport as u16 {
        s = end.saturating_sub(viewport as u16 - 1);
    }
    s.min(max_scroll)
}

enum TimelineItem<'a> {
    Comment(&'a Comment),
    Review(&'a ReviewThread),
    Event(&'a TimelineEvent),
}

impl TimelineItem<'_> {
    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TimelineItem::Comment(c) => c.created,
            TimelineItem::Review(t) => t.comments.first().map_or_else(Utc::now, |c| c.created),
            TimelineItem::Event(e) => e.created,
        }
    }
}

struct TimelineBlock {
    lines: Vec<Line<'static>>,
    reply_to: Option<u64>,
}

struct ItemNav {
    start: usize,
    span: usize,
    reply_to: Option<u64>,
}

fn timeline_count(comments: &[Comment], threads: &[ReviewThread], events: &[TimelineEvent]) -> usize {
    comments.len() + events.len() + threads.iter().filter(|t| !t.comments.is_empty()).count()
}

fn build_blocks(
    comments: &[Comment],
    threads: &[ReviewThread],
    events: &[TimelineEvent],
    diff: Option<&Diff>,
    width: u16,
    focused: usize,
    author: &str,
) -> Vec<TimelineBlock> {
    let mut items: Vec<TimelineItem<'_>> =
        Vec::with_capacity(comments.len() + threads.len() + events.len());
    items.extend(comments.iter().map(TimelineItem::Comment));
    items.extend(threads.iter().map(TimelineItem::Review));
    items.extend(events.iter().map(TimelineItem::Event));
    items.sort_by_key(|item| Reverse(item.timestamp()));

    let now = Utc::now();
    let mut blocks: Vec<TimelineBlock> = Vec::new();
    for item in &items {
        let active = blocks.len() == focused;
        let block = match item {
            TimelineItem::Comment(c) => Some(TimelineBlock {
                lines: super::super::comment::comment_box(c, width, now, active, author),
                reply_to: c.reply_to,
            }),
            TimelineItem::Review(t) => {
                super::super::comment::review_thread_box(t, diff, width, now, active, author)
                    .map(|lines| TimelineBlock {
                        lines,
                        reply_to: t.reply_to,
                    })
            }
            TimelineItem::Event(e) => {
                let (_, lines) = event_block(e, now);
                Some(TimelineBlock {
                    lines,
                    reply_to: None,
                })
            }
        };
        if let Some(block) = block {
            blocks.push(block);
        }
    }
    blocks
}

fn timeline_rail(blocks: Vec<TimelineBlock>) -> (Vec<Line<'static>>, Vec<ItemNav>) {
    let mut all: Vec<Line<'static>> = Vec::new();
    let mut navs: Vec<ItemNav> = Vec::new();
    for (i, block) in blocks.into_iter().enumerate() {
        if i > 0 {
            all.push(Line::raw(""));
        }
        let start = all.len();
        let span = block.lines.len();
        all.extend(block.lines);
        navs.push(ItemNav {
            start,
            span,
            reply_to: block.reply_to,
        });
    }

    if all.is_empty() {
        all.push(Line::default());
    }
    (all, navs)
}

fn event_block(event: &TimelineEvent, now: DateTime<Utc>) -> (Color, Vec<Line<'static>>) {
    let theme = theme::current();

    let header = |verb: String, color: Color| -> Line<'static> {
        let mut lead: Vec<Span<'static>> = Vec::new();
        if let Some(actor) = &event.actor {
            lead.push(Span::styled(
                format!("@{}", actor.username),
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ));
            lead.push(Span::raw(" "));
        }
        lead.push(Span::styled(verb, Style::default().fg(color)));
        widgets::author_line(lead, event.created, now)
    };

    if let EventKind::Pushed(commits) = &event.kind {
        let verb = if commits.len() == 1 {
            "added 1 commit".to_string()
        } else {
            format!("added {} commits", commits.len())
        };
        let mut lines = vec![header(verb, theme.info)];
        for c in commits {
            lines.push(Line::from(vec![
                Span::styled(format!("{}  ", c.id), Style::default().fg(theme.warning)),
                Span::styled(c.message.clone(), Style::default().fg(theme.muted)),
            ]));
        }
        return (theme.info, lines);
    }

    let (verb, color) = match &event.kind {
        EventKind::Opened => ("opened this pull request", theme.info),
        EventKind::Approved => ("approved these changes", theme.success),
        EventKind::ChangesRequested => ("requested changes", theme.error),
        EventKind::ReviewRemoved => ("dismissed their review", theme.muted),
        EventKind::Merged => ("merged this pull request", theme.status_merged),
        EventKind::Declined => ("declined this pull request", theme.status_declined),
        EventKind::Reopened => ("reopened this pull request", theme.info),
        EventKind::Pushed(_) => unreachable!("handled above"),
    };
    (color, vec![header(verb.to_string(), color)])
}
