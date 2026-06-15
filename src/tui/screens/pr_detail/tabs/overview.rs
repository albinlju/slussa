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

const TIMELINE_COL: u16 = 2;
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
    render_timeline(frame, pr_data, ui, body_area, scrollbar_area);
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
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
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

    let content = build_timeline(
        &activity.comments,
        &activity.threads,
        &activity.events,
        diff,
        area.width.saturating_sub(TIMELINE_RIGHT_PAD + TIMELINE_COL),
    );

    let max_scroll = content.len().saturating_sub(area.height as usize) as u16;
    ui.overview_scroll = ui.overview_scroll.min(max_scroll);
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

fn build_timeline(
    comments: &[Comment],
    threads: &[ReviewThread],
    events: &[TimelineEvent],
    diff: Option<&Diff>,
    width: u16,
) -> Vec<Line<'static>> {
    let mut items: Vec<TimelineItem<'_>> =
        Vec::with_capacity(comments.len() + threads.len() + events.len());
    items.extend(comments.iter().map(TimelineItem::Comment));
    items.extend(threads.iter().map(TimelineItem::Review));
    items.extend(events.iter().map(TimelineItem::Event));
    items.sort_by_key(|item| Reverse(item.timestamp()));

    let theme = theme::current();
    let now = Utc::now();
    let blocks: Vec<(Color, Vec<Line<'static>>)> = items
        .iter()
        .filter_map(|item| match item {
            TimelineItem::Comment(c) => Some((
                theme.info,
                super::super::comment::comment_box(c, width, now),
            )),
            TimelineItem::Review(t) => {
                super::super::comment::review_thread_box(t, diff, width, now)
                    .map(|l| (theme.accent, l))
            }
            TimelineItem::Event(e) => Some(event_block(e, now)),
        })
        .collect();

    timeline_rail(blocks)
}

fn timeline_rail(blocks: Vec<(Color, Vec<Line<'static>>)>) -> Vec<Line<'static>> {
    let connector = Span::styled("│ ", Style::default().fg(theme::current().divider));

    let mut all: Vec<Line<'static>> = Vec::new();
    for (i, (color, lines)) in blocks.into_iter().enumerate() {
        if i > 0 {
            all.push(Line::from(connector.clone()));
            all.push(Line::from(connector.clone()));
        }
        let circle = Span::styled(
            "● ",
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        );
        for (j, line) in lines.into_iter().enumerate() {
            let marker = if j == 0 {
                circle.clone()
            } else {
                connector.clone()
            };
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
