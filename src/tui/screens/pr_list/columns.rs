//! The columns of the PR list: which fit, how wide they are and what a row
//! shows in each.
use crate::{
    domain::{
        attention::{Attention, attention},
        ci::CiSummary,
        pr::PullRequest,
        review::{Reviewer, ReviewerState},
        user::Username,
    },
    tui::{
        icons, theme,
        widgets::table::{Cell, Column, Width},
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Style},
    text::Span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListColumn {
    Id,
    Status,
    Author,
    Title,
    Ci,
    Diff,
    Comments,
    Reviews,
    Age,
    /// Why the PR needs the viewer. Shown only while some row has a reason.
    Attention,
}

impl ListColumn {
    /// The columns a list this wide has room for, in the order they are shown.
    /// The title is given its space first; secondary details remain in the PR
    /// view.
    pub(super) const fn visible(width: u16) -> &'static [Self] {
        use ListColumn::{Age, Attention, Author, Ci, Comments, Diff, Id, Reviews, Status, Title};
        match width {
            0..=59 => &[Id, Title],
            60..=89 => &[Id, Title, Author, Ci],
            90..=119 => &[Id, Title, Attention, Author, Ci, Reviews, Age],
            _ => &[
                Id, Title, Attention, Author, Status, Ci, Diff, Comments, Reviews, Age,
            ],
        }
    }

    pub(super) const fn spec(self) -> Column {
        let (title, width) = match self {
            Self::Id => ("#", Width::Fixed(7)),
            Self::Status => ("Status", Width::Fixed(10)),
            Self::Author => ("Author", Width::Fixed(18)),
            Self::Title => ("Title", Width::Flex(1)),
            Self::Ci => ("CI", Width::Fixed(4)),
            Self::Diff => ("Diff", Width::Fixed(12)),
            Self::Comments => ("Comments", Width::Fixed(10)),
            Self::Reviews => ("Reviews", Width::Fixed(9)),
            Self::Age => ("Age", Width::Fixed(8)),
            Self::Attention => ("Needs you", Width::Fixed(19)),
        };
        Column { title, width }
    }

    /// What `pr`'s row shows in this column.
    pub(super) fn cell(self, pr: &PullRequest, viewer: &Username) -> Cell {
        let theme = theme::current();
        let muted = Style::default().fg(theme.muted);
        match self {
            Self::Id => vec![Span::styled(format!("#{}", pr.id), muted)],
            Self::Status => vec![Span::styled(
                pr.status.label().to_string(),
                Style::default().fg(theme.status_color(&pr.status)),
            )],
            Self::Author => vec![Span::styled(
                pr.author.username.clone(),
                Style::default().fg(theme.info),
            )],
            Self::Title => vec![Span::styled(
                pr.title.clone(),
                Style::default().fg(theme.fg),
            )],
            Self::Ci => {
                let (symbol, color) = match pr.ci {
                    CiSummary::Success => (icons::CHECK_CIRCLE, theme.success),
                    CiSummary::Failed => (icons::TIMES_CIRCLE, theme.error),
                    CiSummary::Pending => (icons::CLOCK, theme.warning),
                    // adjust — half circle, neutral/not run
                    CiSummary::Unknown => (icons::ADJUST, theme.muted),
                };
                vec![Span::styled(symbol, Style::default().fg(color))]
            }
            Self::Diff => vec![
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
            Self::Comments => vec![Span::styled(pr.comment_count.to_string(), muted)],
            Self::Reviews => {
                let (text, color) = review_summary(&pr.reviewers);
                vec![Span::styled(text, Style::default().fg(color))]
            }
            Self::Age => vec![Span::styled(age_label(pr.created), muted)],
            Self::Attention => attention_cell(attention(pr, viewer)),
        }
    }
}

fn attention_cell(reason: Option<Attention>) -> Cell {
    let theme = theme::current();
    reason.map_or_else(Vec::new, |reason| {
        let color = match reason {
            Attention::ChangesRequested | Attention::CiFailed => theme.error,
            Attention::ReviewRequested => theme.warning,
            Attention::Approved => theme.success,
        };
        vec![Span::styled(reason.label(), Style::default().fg(color))]
    })
}

fn age_label(created: DateTime<Utc>) -> String {
    match (Utc::now() - created).num_days() {
        0 => "today".to_string(),
        1 => "1d".to_string(),
        days => format!("{days}d"),
    }
}

fn review_summary(reviewers: &[Reviewer]) -> (String, Color) {
    let theme = theme::current();
    if reviewers.is_empty() {
        return ("—".to_string(), theme.muted);
    }
    let approved = reviewers
        .iter()
        .filter(|r| r.state == ReviewerState::Approved)
        .count();
    let total = reviewers.len();
    let any_blocking = reviewers
        .iter()
        .any(|r| r.state == ReviewerState::ChangesRequested);
    let color = if any_blocking {
        theme.error
    } else if approved == total {
        theme.success
    } else {
        theme.warning
    };
    (format!("{approved}/{total}"), color)
}
