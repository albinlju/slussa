use crate::{
    domain::{pr::PullRequest, review::ReviewerState},
    tui::{
        app::store::{LoadState, PrData},
        ui::{format, icons, theme, widgets},
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

pub struct Sidebar<'a> {
    pub pr: &'a PullRequest,
    pub data: Option<&'a PrData>,
    pub show_builds: bool,
    pub ai: Option<AiReview<'a>>,
}

/// What an agent's review of the PR says, for the sidebar.
#[derive(Debug, Clone, Copy)]
pub struct AiReview<'a> {
    /// Who wrote the summary.
    pub agent: Option<&'a str>,
    /// What it says of the PR as a whole; none when only comments were handed in.
    pub text: Option<&'a str>,
    /// Whether it is of an older commit than the branch is at now.
    pub older: bool,
    /// How many proposals wait for the reader.
    pub open: usize,
}

/// How many rows of a summary the sidebar gives, before it says there is more.
const SUMMARY_ROWS: usize = 10;
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

        if let Some(ai) = self.ai {
            section_heading(&mut lines, "AI review");
            lines.extend(ai_review(ai, inner.width as usize));
            lines.push(Line::default());
        }

        if self.show_builds {
            section_heading(&mut lines, "Builds");
            lines.extend(builds_summary(pr_data));
            lines.push(Line::default());
        }

        let labels = labels(pr, pr_data);
        if !labels.is_empty() {
            section_heading(&mut lines, "Labels");
            lines.extend(labels);
            lines.push(Line::default());
        }

        let issues = issues(pr_data);
        if !issues.is_empty() {
            section_heading(&mut lines, "Closes");
            lines.extend(issues);
            lines.push(Line::default());
        }

        section_heading(&mut lines, "Details");
        lines.extend(details(pr, now, inner.width as usize));

        let lines: Vec<_> = lines
            .into_iter()
            .map(|line| Line::from(widgets::truncate_to_width(line.spans, inner.width as usize)))
            .collect();
        Paragraph::new(lines).render(inner, buf);
    }
}

fn ai_review(ai: AiReview<'_>, width: usize) -> Vec<Line<'static>> {
    let theme = theme::current();
    let mut lines = Vec::new();
    if let Some(agent) = ai.agent {
        lines.push(Line::from(vec![
            Span::styled(
                "[AI] ",
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ),
            Span::styled(agent.to_owned(), Style::default().fg(theme.info)),
        ]));
    }
    if ai.older {
        lines.push(muted_line("of an older commit"));
    }
    if let Some(text) = ai.text {
        let rows = widgets::wrap_text(text, width);
        let more = rows.len() > SUMMARY_ROWS;
        lines.extend(
            rows.into_iter()
                .take(SUMMARY_ROWS)
                .map(|row| Line::from(Span::styled(row, Style::default().fg(theme.fg)))),
        );
        if more {
            lines.push(muted_line("…"));
        }
    }
    if ai.open > 0 {
        lines.push(Line::from(Span::styled(
            format!("{} proposed · Diff tab", ai.open),
            Style::default().fg(theme.accent),
        )));
    } else if ai.text.is_some() {
        lines.push(muted_line("Nothing left to decide"));
    }
    lines
}

fn section_heading(lines: &mut Vec<Line<'static>>, title: &str) {
    let theme = theme::current();
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(theme.muted),
    )));
}

fn reviewers(pr: &PullRequest) -> Vec<Line<'static>> {
    let theme = theme::current();
    if pr.reviewers.is_empty() {
        return vec![muted_line("No reviewers")];
    }
    pr.reviewers
        .iter()
        .map(|r| {
            let (icon, color) = match r.state {
                ReviewerState::Approved => (icons::CHECK_CIRCLE, theme.success),
                ReviewerState::ChangesRequested => (icons::TIMES_CIRCLE, theme.error),
                ReviewerState::Commented | ReviewerState::Requested => {
                    (icons::CIRCLE_O, theme.muted)
                }
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
            use crate::tui::ui::screens::pr_detail::build_status::{OverallState, build_stats};
            let stats = build_stats(builds);
            let (icon, _) = OverallState::of(&stats).glyph();
            vec![Line::from(vec![
                Span::styled(format!("{icon} "), Style::default().fg(stats.accent())),
                Span::styled(
                    format!("{}/{} passing", stats.passing, stats.total),
                    Style::default()
                        .fg(theme::current().fg)
                        .add_modifier(Modifier::BOLD),
                ),
            ])]
        }
        Some(LoadState::Loaded(_)) => vec![muted_line("No builds")],
        Some(LoadState::Failed(_)) => vec![muted_line("Unavailable")],
        _ => vec![widgets::loading("Loading…")],
    }
}

/// The labels read for this PR when the provider's list leaves them out,
/// otherwise the list's own.
fn labels(pr: &PullRequest, pr_data: Option<&PrData>) -> Vec<Line<'static>> {
    let accent = Style::default().fg(theme::current().decorative);
    let labels = match pr_data.map(|data| &data.info) {
        Some(LoadState::Loaded(info)) => &info.labels,
        _ => &pr.labels,
    };
    labels
        .iter()
        .map(|label| Line::from(Span::styled(label.clone(), accent)))
        .collect()
}

/// The issues the PR closes, once its info is read.
fn issues(pr_data: Option<&PrData>) -> Vec<Line<'static>> {
    let theme = theme::current();
    let Some(LoadState::Loaded(info)) = pr_data.map(|data| &data.info) else {
        return Vec::new();
    };
    info.issues
        .iter()
        .map(|issue| {
            Line::from(vec![
                Span::styled(
                    format!("#{}", issue.number),
                    Style::default().fg(theme.info),
                ),
                Span::raw(" "),
                Span::styled(issue.title.clone(), Style::default().fg(theme.fg)),
            ])
        })
        .collect()
}

fn details(pr: &PullRequest, now: DateTime<Utc>, width: usize) -> Vec<Line<'static>> {
    let theme = theme::current();
    let fg = Style::default().fg(theme.fg);
    let detail = |key: &str, value: Vec<Span<'static>>| -> Line<'static> {
        widgets::fitted_row(
            vec![Span::styled(
                key.to_owned(),
                Style::default().fg(theme.muted),
            )],
            value,
            width,
        )
    };
    vec![
        detail(
            "Opened",
            vec![Span::styled(format::relative_age(pr.created, now), fg)],
        ),
        detail(
            "Updated",
            vec![Span::styled(format::relative_age(pr.updated, now), fg)],
        ),
        detail(
            "Diff",
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
            "Files",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_summary_keeps_one_row_in_all_loading_states() {
        assert_eq!(builds_summary(None).len(), 1);
        let mut data = PrData::default();
        for builds in [
            LoadState::Loading,
            LoadState::Failed(crate::providers::FetchError::Network("offline".into())),
            LoadState::Loaded(vec![]),
            LoadState::Loaded(vec![crate::domain::ci::Build {
                name: "Tests".into(),
                state: crate::domain::ci::BuildState::Successful,
                duration_ms: None,
                log: None,
            }]),
        ] {
            data.builds = builds;
            assert_eq!(builds_summary(Some(&data)).len(), 1);
        }
    }
}
