use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use crate::{
    app::state::{LoadState, PrData, UiMemory},
    domain::{
        comment::{Comment, ReviewThread, split_suggestions},
        diff::{Diff, DiffLine},
        event::{EventKind, TimelineEvent},
        pr::PullRequest,
        review::ReviewerState,
    },
    tui::{
        format, markdown,
        theme::{self, Theme},
        widgets,
    },
};

/// Width of the left timeline column (`● ` or `│ ` — glyph + a trailing
/// space before content).
const TIMELINE_COL: u16 = 2;

const SIDEBAR_WIDTH: u16 = 30;
/// Below this width the sidebar would squeeze the timeline into uselessness,
/// so it's dropped entirely.
const SIDEBAR_BREAKPOINT: u16 = 64;

pub fn render(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    area: Rect,
) {
    let (timeline_area, sidebar_area) = if area.width >= SIDEBAR_BREAKPOINT {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(SIDEBAR_WIDTH)])
            .split(area);
        (chunks[0], Some(chunks[1]))
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

fn dim(text: &str) -> Line<'static> {
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
        Some(LoadState::Loaded(_)) => vec![dim("no builds")],
        Some(LoadState::Failed(_)) => vec![dim("unavailable")],
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
        lines.push(dim("—"));
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

    // Bitbucket DC has no labels; the section is hidden when empty.
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
    let theme = theme::current();
    let Some(bundle) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.activity), "activity", area)
    else {
        return;
    };

    if bundle.comments.is_empty() && bundle.threads.is_empty() && bundle.events.is_empty() {
        let p = Paragraph::new("(no activity)").style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    let diff = pr_data.and_then(|d| match &d.diff {
        LoadState::Loaded(diff) => Some(diff),
        _ => None,
    });

    // The scrollbar owns the rightmost column.
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

enum Event<'a> {
    Issue(&'a Comment),
    Review(&'a ReviewThread),
    Activity(&'a TimelineEvent),
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
            Event::Activity(e) => e.created,
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
    let mut events: Vec<Event<'_>> =
        Vec::with_capacity(comments.len() + threads.len() + activity.len());
    events.extend(comments.iter().map(Event::Issue));
    events.extend(threads.iter().map(Event::Review));
    events.extend(activity.iter().map(Event::Activity));
    // Newest first.
    events.sort_by_key(|e| std::cmp::Reverse(e.timestamp()));

    let now = Utc::now();

    // Content lines first, then the timeline column gets prepended: ● on each
    // event's first line, │ everywhere else.
    let blocks: Vec<(EventStyle, Vec<Line<'static>>)> = events
        .iter()
        .filter_map(|event| match event {
            Event::Issue(c) => Some((EventStyle::Comment, build_issue_lines(c, width, now))),
            Event::Review(t) => {
                build_review_lines(t, diff, width, now).map(|l| (EventStyle::Review, l))
            }
            Event::Activity(e) => {
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
    /// Lifecycle event, carrying its own dot color.
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

/// One `@actor <verb> · age` line per lifecycle event (a push adds one line
/// per commit). Returns the dot color so the timeline circle matches.
fn activity_lines(event: &TimelineEvent, now: DateTime<Utc>) -> (Color, Vec<Line<'static>>) {
    let theme = theme::current();
    let age = format::relative_age(event.created, now);

    let header = |verb: String, color: Color| -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if let Some(actor) = &event.actor {
            spans.push(Span::styled(
                format!("@{}", actor.username),
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(verb, Style::default().fg(color)));
        spans.push(Span::styled(
            format!(" · {age}"),
            Style::default().fg(theme.muted),
        ));
        Line::from(spans)
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

fn build_issue_lines(c: &Comment, width: u16, now: DateTime<Utc>) -> Vec<Line<'static>> {
    let text_width = widgets::box_text_width(width);
    let header = issue_comment_header(&c.author.username, c.created, now);
    let mut body = markdown::render_flush(&c.content, text_width);
    if let Some(line) = widgets::reactions_line(&c.reactions) {
        body.push(Line::default());
        body.push(line);
    }
    widgets::boxed(header, body, width, theme::current().divider)
}

fn build_review_lines(
    thread: &ReviewThread,
    diff: Option<&Diff>,
    width: u16,
    now: DateTime<Utc>,
) -> Option<Vec<Line<'static>>> {
    let theme = theme::current();
    let first = thread.comments.first()?;
    let text_width = widgets::box_text_width(width);
    let header = issue_comment_header(&first.author.username, first.created, now);

    let mut body: Vec<Line<'static>> = Vec::new();

    // Nested box: path:line as header, diff snippet as body.
    let location = match thread.line.or(thread.old_line) {
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
    let inner_header = Line::from(anchor_spans);

    let inner_text_width = widgets::box_text_width(text_width);
    let (snippet, anchor_text) = diff
        .map(|d| diff_snippet(d, &thread.path, thread.line, thread.old_line, inner_text_width))
        .unwrap_or_default();
    let splits: Vec<(String, Vec<String>)> = thread
        .comments
        .iter()
        .map(|c| split_suggestions(&c.content))
        .collect();
    // A suggestion box repeats the anchored line as its `-` side, so the
    // snippet would show the same line twice — keep just the location header.
    let has_suggestion = splits.iter().any(|(_, s)| !s.is_empty());
    if snippet.is_empty() || has_suggestion {
        body.push(inner_header);
    } else {
        body.extend(widgets::boxed(inner_header, snippet, text_width, theme.divider));
    }
    let anchor = thread.line.or(thread.old_line).zip(anchor_text.as_deref());

    // The first author is already in the box header; replies get a `↳ @user`
    // line.
    for (i, (comment, (prose, suggestions))) in
        thread.comments.iter().zip(splits.iter()).enumerate()
    {
        if i > 0 {
            body.push(Line::raw(""));
            let age = format::relative_age(comment.created, now);
            body.push(Line::from(vec![
                Span::styled(
                    format!("↳ @{}", comment.author.username),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" · {age}"), Style::default().fg(theme.muted)),
            ]));
        }
        if !prose.trim().is_empty() {
            body.extend(markdown::render_flush(prose, text_width));
        }
        for suggestion in suggestions {
            body.push(Line::default());
            body.extend(super::suggestion_lines(anchor, suggestion, text_width));
        }
        if let Some(line) = widgets::reactions_line(&comment.reactions) {
            body.push(Line::default());
            body.push(line);
        }
    }

    Some(widgets::boxed(header, body, width, theme.divider))
}

fn issue_comment_header(
    username: &str,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Line<'static> {
    let theme = theme::current();
    let age = format::relative_age(created, now);
    Line::from(vec![
        Span::styled(
            username.to_string(),
            Style::default()
                .fg(theme.info)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" commented", Style::default().fg(theme.muted)),
        Span::styled(format!(" · {age}"), Style::default().fg(theme.muted)),
    ])
}

/// Leading context lines above the anchored line.
const SNIPPET_CONTEXT: usize = 3;

/// A diff snippet around the thread's anchor line, styled to match the Diff
/// tab, plus the anchored line's text (the `-` side of a suggested change).
/// Empty when the file/line isn't in the loaded diff (outdated comment, or
/// not loaded yet).
fn diff_snippet(
    diff: &Diff,
    path: &str,
    line: Option<usize>,
    old_line: Option<usize>,
    width: u16,
) -> (Vec<Line<'static>>, Option<String>) {
    let theme = theme::current();
    let Some(file) = diff.files.iter().find(|f| f.path == path) else {
        return (Vec::new(), None);
    };

    struct Row<'a> {
        dl: &'a DiffLine,
        new_no: usize,
        old_no: usize,
    }
    let mut rows: Vec<Row> = Vec::new();
    for hunk in &file.hunks {
        let mut new_no = hunk.new_start;
        let mut old_no = hunk.old_start;
        for dl in &hunk.lines {
            rows.push(Row { dl, new_no, old_no });
            match dl {
                DiffLine::Added(_) => new_no += 1,
                DiffLine::Removed(_) => old_no += 1,
                DiffLine::Context(_) => {
                    new_no += 1;
                    old_no += 1;
                }
            }
        }
    }

    // Added/context lines anchor by new-side number, removed by old-side.
    let anchor = rows.iter().position(|r| match (line, old_line) {
        (Some(l), _) => !matches!(r.dl, DiffLine::Removed(_)) && r.new_no == l,
        (None, Some(o)) => matches!(r.dl, DiffLine::Removed(_)) && r.old_no == o,
        _ => false,
    });
    let Some(anchor) = anchor else {
        return (Vec::new(), None);
    };
    let anchor_text = match rows[anchor].dl {
        DiffLine::Added(c) | DiffLine::Removed(c) | DiffLine::Context(c) => c.clone(),
    };

    let window = &rows[anchor.saturating_sub(SNIPPET_CONTEXT)..=anchor];
    let num_width = window
        .iter()
        .map(|r| r.new_no)
        .max()
        .unwrap_or(1)
        .to_string()
        .len();
    let row_w = width as usize;

    let lines: Vec<Line<'static>> = window
        .iter()
        .map(|r| match r.dl {
            DiffLine::Added(c) => widgets::numbered_diff_row(
                Some(r.new_no as u32),
                num_width,
                "+",
                c,
                theme.diff_added,
                Some(theme.diff_added_bg),
                theme.fg,
                row_w,
            ),
            DiffLine::Removed(c) => widgets::numbered_diff_row(
                None,
                num_width,
                "-",
                c,
                theme.diff_removed,
                Some(theme.diff_removed_bg),
                theme.muted,
                row_w,
            ),
            DiffLine::Context(c) => widgets::numbered_diff_row(
                Some(r.new_no as u32),
                num_width,
                " ",
                c,
                theme.muted,
                None,
                theme.diff_context,
                row_w,
            ),
        })
        .collect();
    (lines, Some(anchor_text))
}

