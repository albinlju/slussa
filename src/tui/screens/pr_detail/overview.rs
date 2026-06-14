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
    tui::{
        format, layout,
        theme::{self, Theme},
        widgets,
    },
};

const TIMELINE_COL: u16 = 2;

const SIDEBAR_WIDTH: u16 = 30;
const SIDEBAR_BREAKPOINT: u16 = 64;

pub fn render(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    area: Rect,
) {
    let (timeline_area, sidebar_area) = if area.width >= SIDEBAR_BREAKPOINT {
        let [timeline, sidebar] = layout::split(
            area,
            Direction::Horizontal,
            [Constraint::Min(0), Constraint::Length(SIDEBAR_WIDTH)],
        );
        (timeline, Some(sidebar))
    } else {
        (area, None)
    };

    if let Some(sidebar) = sidebar_area {
        render_sidebar(frame, pr, pr_data, sidebar);
    }
    render_timeline(frame, pr_data, ui, timeline_area);
}

fn section_heading(lines: &mut Vec<Line<'static>>, title: &str) {
    let theme = theme::current();
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(theme.muted).add_modifier(Modifier::BOLD),
    )));
}

fn muted_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(theme::current().muted),
    ))
}

fn builds_summary(pr_data: Option<&PrData>) -> Vec<Line<'static>> {
    match pr_data.map(|d| &d.builds) {
        Some(LoadState::Loaded(builds)) if !builds.is_empty() => {
            let stats = super::checks::build_stats(builds);
            vec![
                Line::from(Span::styled(
                    format!("{}/{} passing", stats.passing, stats.total),
                    Style::default()
                        .fg(stats.accent())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(super::checks::progress_bar(builds)),
            ]
        }
        Some(LoadState::Loaded(_)) => vec![muted_line("no builds")],
        Some(LoadState::Failed(_)) => vec![muted_line("unavailable")],
        _ => vec![widgets::loading("loading…")],
    }
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
    if pr.reviewers.is_empty() {
        lines.push(muted_line("—"));
    } else {
        for r in &pr.reviewers {
            let (icon, color) = match r.state {
                ReviewerState::Approved => ("\u{f058}", theme.success), //  check-circle
                ReviewerState::ChangesRequested => ("\u{f057}", theme.error), //  times-circle
                ReviewerState::Commented => ("\u{f10c}", theme.muted),  //  circle-o
            };
            lines.push(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(
                    format!("@{}", r.author.username),
                    Style::default().fg(theme.info),
                ),
            ]));
        }
    }
    lines.push(Line::default());

    section_heading(&mut lines, "Builds");
    lines.extend(builds_summary(pr_data));
    lines.push(Line::default());

    if !pr.labels.is_empty() {
        section_heading(&mut lines, "Labels");
        for label in &pr.labels {
            lines.push(Line::from(Span::styled(
                label.clone(),
                Style::default().fg(theme.accent),
            )));
        }
        lines.push(Line::default());
    }

    section_heading(&mut lines, "Details");
    let detail = |key: &str, value: Vec<Span<'static>>| -> Line<'static> {
        let mut spans = vec![Span::styled(
            format!("{key:<8}"),
            Style::default().fg(theme.muted),
        )];
        spans.extend(value);
        Line::from(spans)
    };
    let fg = Style::default().fg(theme.fg);
    lines.push(detail(
        "opened",
        vec![Span::styled(format::relative_age(pr.created, now), fg)],
    ));
    lines.push(detail(
        "updated",
        vec![Span::styled(format::relative_age(pr.updated, now), fg)],
    ));
    lines.push(detail(
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
    ));
    lines.push(detail(
        "files",
        vec![Span::styled(pr.changed_files.to_string(), fg)],
    ));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_timeline(frame: &mut Frame, pr_data: Option<&PrData>, ui: &mut UiMemory, area: Rect) {
    let Some(bundle) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.activity), "activity", area)
    else {
        return;
    };

    if bundle.comments.is_empty() && bundle.threads.is_empty() && bundle.events.is_empty() {
        frame.render_widget(widgets::empty_state("(no activity)"), area);
        return;
    }

    let diff = pr_data.and_then(|d| match &d.diff {
        LoadState::Loaded(diff) => Some(diff),
        _ => None,
    });

    let lines = build_overview_lines(
        &bundle.comments,
        &bundle.threads,
        &bundle.events,
        diff,
        area.width.saturating_sub(1 + TIMELINE_COL),
    );
    widgets::scrolled_paragraph(
        frame,
        lines,
        &mut ui.overview_scroll,
        &mut ui.overview_viewport,
        area,
    );
}

enum TimelineItem<'a> {
    Comment(&'a Comment),
    Review(&'a ReviewThread),
    Activity(&'a TimelineEvent),
}

impl TimelineItem<'_> {
    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TimelineItem::Comment(c) => c.created,
            TimelineItem::Review(t) => t
                .comments
                .first().map_or_else(Utc::now, |c| c.created),
            TimelineItem::Activity(e) => e.created,
        }
    }
}

fn build_overview_lines(
    comments: &[Comment],
    threads: &[ReviewThread],
    activity: &[TimelineEvent],
    diff: Option<&Diff>,
    width: u16,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let mut events: Vec<TimelineItem<'_>> =
        Vec::with_capacity(comments.len() + threads.len() + activity.len());
    events.extend(comments.iter().map(TimelineItem::Comment));
    events.extend(threads.iter().map(TimelineItem::Review));
    events.extend(activity.iter().map(TimelineItem::Activity));
    events.sort_by_key(|e| std::cmp::Reverse(e.timestamp()));

    let now = Utc::now();

    let blocks: Vec<(EventStyle, Vec<Line<'static>>)> = events
        .iter()
        .filter_map(|event| match event {
            TimelineItem::Comment(c) => {
                Some((EventStyle::Comment, super::comment::comment_box(c, width, now)))
            }
            TimelineItem::Review(t) => super::comment::review_thread_box(t, diff, width, now)
                .map(|l| (EventStyle::Review, l)),
            TimelineItem::Activity(e) => {
                let (color, lines) = activity_lines(e, now);
                Some((EventStyle::Activity(color), lines))
            }
        })
        .collect();

    let connector_style = Style::default().fg(theme.divider);
    let connector = Span::styled("│ ", connector_style);

    let mut all: Vec<Line<'static>> = Vec::new();
    for (i, (style, lines)) in blocks.into_iter().enumerate() {
        if i > 0 {
            all.push(Line::from(connector.clone()));
            all.push(Line::from(connector.clone()));
        }
        let circle = Span::styled(
            "● ",
            Style::default()
                .fg(style.color(theme))
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
    Comment,
    Review,
    Activity(Color),
}

impl EventStyle {
    fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Comment => theme.info,
            Self::Review => theme.accent,
            Self::Activity(c) => c,
        }
    }
}

fn activity_lines(event: &TimelineEvent, now: DateTime<Utc>) -> (Color, Vec<Line<'static>>) {
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
