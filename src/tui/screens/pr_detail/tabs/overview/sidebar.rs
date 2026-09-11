use crate::{
    app::store::{LoadState, PrData},
    domain::{pr::PullRequest, review::ReviewerState},
    tui::{format, icons, theme, widgets},
};
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Widget, Wrap},
};

pub struct Sidebar<'a> {
    pub pr: &'a PullRequest,
    pub data: Option<&'a PrData>,
}
impl Widget for Sidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let pr = self.pr;
        let pr_data = self.data;
        let theme = theme::current();
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(theme.divider))
            .padding(Padding::horizontal(1));
        let inner = block.inner(area);
        block.render(area, buf);

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

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
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
            let stats = crate::tui::screens::pr_detail::build_status::build_stats(builds);
            vec![
                Line::from(Span::styled(
                    format!("{}/{} passing", stats.passing, stats.total),
                    Style::default()
                        .fg(stats.accent())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(crate::tui::screens::pr_detail::build_status::progress_bar(
                    builds,
                )),
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
