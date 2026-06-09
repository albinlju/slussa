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
        ci::BuildState,
        comment::{Comment, ReviewThread},
        diff::{Diff, DiffLine},
        event::{EventKind, TimelineEvent},
        pr::PullRequest,
        review::ReviewerState,
    },
    tui::{
        theme::{self, Theme},
        widgets,
    },
};

/// Width of the left timeline column (`● ` or `│ ` — glyph + a trailing
/// space before content).
const TIMELINE_COL: u16 = 2;

const SIDEBAR_WIDTH: u16 = 30;

pub fn render(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut UiMemory,
    area: Rect,
) {
    // Wide enough → split off a right-hand metadata sidebar; otherwise the
    // conversation timeline takes the full width.
    let (timeline_area, sidebar_area) = if area.width >= 64 {
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

/// `3/5 passing` on the first line, the Builds tab's colored progress bar on
/// the second. Spinner while loading, dash when there are none.
fn builds_summary(pr_data: Option<&PrData>) -> Vec<Line<'static>> {
    let theme = theme::current();
    match pr_data.map(|d| &d.builds) {
        Some(LoadState::Loaded(builds)) if !builds.is_empty() => {
            let total = builds.len();
            let passing = builds
                .iter()
                .filter(|b| b.state == BuildState::Successful)
                .count();
            let any_running = builds.iter().any(|b| b.state == BuildState::InProgress);
            let any_failed = builds
                .iter()
                .any(|b| matches!(b.state, BuildState::Failed | BuildState::Cancelled));
            let accent = if any_running {
                theme.warning
            } else if any_failed {
                theme.error
            } else if passing == total {
                theme.success
            } else {
                theme.muted
            };
            vec![
                Line::from(Span::styled(
                    format!("{passing}/{total} passing"),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                )),
                Line::from(super::checks::progress_bar(builds)),
            ]
        }
        Some(LoadState::Loaded(_)) => vec![dim("no builds")],
        Some(LoadState::Failed(_)) => vec![dim("unavailable")],
        _ => vec![Line::from(Span::styled(
            format!("{} loading…", widgets::spinner_frame()),
            Style::default().fg(theme.warning),
        ))],
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
            // Status to the LEFT of the name: approved / denied / not responded.
            let (icon, color) = match r.state {
                ReviewerState::Approved => ("\u{f058}", theme.success), //  check-circle
                ReviewerState::ChangesRequested => ("\u{f057}", theme.error), //  times-circle
                // No approve/reject decision yet.
                ReviewerState::Commented => ("\u{f10c}", theme.muted), //  circle-o
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

    // Labels only render when the provider supplies them (GitHub); Bitbucket
    // DC has none, so the section is hidden entirely.
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
        vec![Span::styled(widgets::relative_age(pr.created, now), fg)],
    ));
    lines.push(detail(
        "updated",
        vec![Span::styled(widgets::relative_age(pr.updated, now), fg)],
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
    let bundle = match pr_data.map(|d| &d.activity) {
        Some(LoadState::Loaded(b)) => b,
        Some(LoadState::Failed(msg)) => {
            let p = Paragraph::new(format!("Couldn't load activity: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(p, area);
            return;
        }
        _ => {
            let p = Paragraph::new(format!("{}  Loading...", widgets::spinner_frame()))
                .style(Style::default().fg(theme.warning));
            frame.render_widget(p, area);
            return;
        }
    };

    if bundle.comments.is_empty() && bundle.threads.is_empty() && bundle.events.is_empty() {
        let p = Paragraph::new("(no activity)").style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    // Inline review snippets are pulled from the loaded diff (same source +
    // styling for every provider).
    let diff = pr_data.and_then(|d| match &d.diff {
        LoadState::Loaded(diff) => Some(diff),
        _ => None,
    });

    // Reserve rightmost column for the scrollbar so wrapped markdown doesn't
    // get clipped or overlap the thumb.
    let content_width = area.width.saturating_sub(1);
    let lines = build_overview_lines(
        &bundle.comments,
        &bundle.threads,
        &bundle.events,
        diff,
        content_width.saturating_sub(TIMELINE_COL),
    );

    let total = lines.len();
    let visible = area.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;
    let scroll = ui.overview_scroll.min(max_scroll);
    ui.overview_scroll = scroll;
    ui.overview_viewport = area.height;

    let content_area = Rect { width: content_width, ..area };
    let p = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(p, content_area);

    if max_scroll > 0 {
        let bar = widgets::scrollbar(scroll, max_scroll, area.height);
        frame.render_widget(Paragraph::new(bar), widgets::scrollbar_area(area));
    }
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
    // Newest first — most recent activity sits at the top of the timeline.
    events.sort_by_key(|e| std::cmp::Reverse(e.timestamp()));

    let now = Utc::now();

    // Render each event's content lines first (no left column), then assemble
    // with the timeline column prepended — circle on the first line of each
    // event, vertical connector on all other lines and on the gap rows
    // between events.
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
    /// Plain issue/discussion comment.
    Comment,
    /// Inline review comment anchored to a diff line.
    Review,
    /// Lifecycle event (approved, merged, …) — carries its own dot color.
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

/// Lifecycle rows for the timeline. Most events are one line
/// (`@actor approved · 2d ago`); a push expands to a header plus one line per
/// added commit. Returns the dot color so the timeline circle matches.
fn activity_lines(event: &TimelineEvent, now: DateTime<Utc>) -> (Color, Vec<Line<'static>>) {
    let theme = theme::current();
    let age = widgets::relative_age(event.created, now);

    // `@actor <verb> · age`, with the actor omitted when unattributed.
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
        EventKind::ReadyForReview => ("marked this ready for review", theme.accent),
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
    // Glamour's Dark theme adds a 2-col document margin to every rendered
    // line. Stripping it pulls the comment text flush against the box's
    // inner padding instead of sitting another two cols in.
    let body = widgets::trim_blank_lines(widgets::strip_glamour_margin(
        widgets::markdown(&c.content, text_width + 2),
        2,
    ));
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

    // Nested box: path:line as header, diff snippet as body. Sits inside the
    // outer comment box; comment text follows underneath.
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
    let snippet = diff
        .map(|d| diff_snippet(d, &thread.path, thread.line, thread.old_line, inner_text_width))
        .unwrap_or_default();
    if snippet.is_empty() {
        // No diff context (diff not loaded, or an outdated comment) — show just
        // the location.
        body.push(inner_header);
    } else {
        body.extend(widgets::boxed(inner_header, snippet, text_width, theme.divider));
    }

    // First comment body + replies. The first author is already in the box
    // header so we skip the per-comment header for them; replies still get
    // a `↳ @user` line.
    for (i, comment) in thread.comments.iter().enumerate() {
        if i > 0 {
            body.push(Line::raw(""));
            let age = widgets::relative_age(comment.created, now);
            body.push(Line::from(vec![
                Span::styled(
                    format!("↳ @{}", comment.author.username),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" · {age}"), Style::default().fg(theme.muted)),
            ]));
        }
        body.extend(widgets::trim_blank_lines(widgets::strip_glamour_margin(
            widgets::markdown(&comment.content, text_width + 2),
            2,
        )));
    }

    Some(widgets::boxed(header, body, width, theme.divider))
}

fn issue_comment_header(
    username: &str,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Line<'static> {
    let theme = theme::current();
    let age = widgets::relative_age(created, now);
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

/// Diff hunk styled to match the Diff tab — full-row bg tint on added/removed
/// lines, strong-color prefix for `+`/`-`, plain muted for context. Each row
/// is prefixed with its new-side line number (blank for removed lines).
/// Number of leading context lines to show above the anchored line.
const SNIPPET_CONTEXT: usize = 3;

/// A diff snippet around the line a review thread is anchored to, pulled from
/// the already-loaded diff. Same source + styling for every provider, so
/// threads look identical regardless of backend. Empty when the file/line
/// isn't in the diff (e.g. an outdated comment, or the diff isn't loaded yet).
fn diff_snippet(
    diff: &Diff,
    path: &str,
    line: Option<usize>,
    old_line: Option<usize>,
    width: u16,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let Some(file) = diff.files.iter().find(|f| f.path == path) else {
        return Vec::new();
    };

    // Flatten the file's diff lines, tracking both sides' line numbers.
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

    // Locate the anchored line: by new-side number for added/context, by
    // old-side number for removed.
    let anchor = rows.iter().position(|r| match (line, old_line) {
        (Some(l), _) => !matches!(r.dl, DiffLine::Removed(_)) && r.new_no == l,
        (None, Some(o)) => matches!(r.dl, DiffLine::Removed(_)) && r.old_no == o,
        _ => false,
    });
    let Some(anchor) = anchor else {
        return Vec::new();
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
            DiffLine::Added(c) => numbered_diff_row(
                Some(r.new_no as u32),
                num_width,
                "+",
                c,
                theme.diff_added,
                Some(theme.diff_added_bg),
                theme.fg,
                row_w,
            ),
            DiffLine::Removed(c) => numbered_diff_row(
                None,
                num_width,
                "-",
                c,
                theme.diff_removed,
                Some(theme.diff_removed_bg),
                theme.muted,
                row_w,
            ),
            DiffLine::Context(c) => numbered_diff_row(
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
    lines
}

#[allow(clippy::too_many_arguments)]
fn numbered_diff_row(
    line_num: Option<u32>,
    num_width: usize,
    prefix: &'static str,
    content: &str,
    prefix_fg: Color,
    bg: Option<Color>,
    text_fg: Color,
    row_w: usize,
) -> Line<'static> {
    let num_str = match line_num {
        Some(n) => format!("{n:>num_width$}"),
        None => " ".repeat(num_width),
    };
    // `{num} {prefix} {content}` — single space between each.
    let gutter = format!(" {num_str} ");
    let visible = gutter.chars().count() + prefix.chars().count() + 1 + content.chars().count();
    let pad = row_w.saturating_sub(visible);

    let gutter_style = match bg {
        Some(bg) => Style::default().fg(theme::current().muted).bg(bg),
        None => Style::default().fg(theme::current().muted),
    };
    let prefix_style = {
        let s = Style::default()
            .fg(prefix_fg)
            .add_modifier(Modifier::BOLD);
        match bg {
            Some(bg) => s.bg(bg),
            None => s,
        }
    };
    let text_style = match bg {
        Some(bg) => Style::default().fg(text_fg).bg(bg),
        None => Style::default().fg(text_fg),
    };

    Line::from(vec![
        Span::styled(gutter, gutter_style),
        Span::styled(prefix, prefix_style),
        Span::styled(format!(" {content}{}", " ".repeat(pad)), text_style),
    ])
}
