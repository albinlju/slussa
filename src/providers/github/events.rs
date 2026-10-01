use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::event::{EventKind, TimelineEvent};
use crate::domain::pr::PrId;
use crate::domain::user::User;
use crate::providers::error::FetchError;
use crate::providers::github::cli::run_gh_json;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewAuthor {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReview {
    #[serde(default)]
    author: Option<GhReviewAuthor>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    submitted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrTimeline {
    author: GhReviewAuthor,
    created_at: DateTime<Utc>,
    #[serde(default)]
    merged_at: Option<DateTime<Utc>>,
    #[serde(default)]
    closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    reviews: Vec<GhReview>,
}

pub fn fetch_events(pr_number: PrId) -> Result<Vec<TimelineEvent>, FetchError> {
    let pr_arg = pr_number.to_string();
    let mut pr: GhPrTimeline = run_gh_json(&[
        "pr",
        "view",
        &pr_arg,
        "--json",
        "author,createdAt,mergedAt,closedAt,state",
    ])?;

    pr.reviews =
        super::pagination::pr_nodes(pr_number, "reviews", "author { login } state submittedAt")?;

    let mut out: Vec<TimelineEvent> = Vec::new();

    out.push(TimelineEvent {
        actor: Some(login_user(pr.author.login)),
        kind: EventKind::Opened,
        created: pr.created_at,
    });

    for review in pr.reviews {
        let Some(kind) = review_kind(&review.state) else {
            continue;
        };
        let Some(created) = review.submitted_at else {
            continue;
        };
        out.push(TimelineEvent {
            actor: review.author.map(|a| login_user(a.login)),
            kind,
            created,
        });
    }

    if pr.state == "MERGED"
        && let Some(created) = pr.merged_at
    {
        out.push(TimelineEvent {
            actor: None,
            kind: EventKind::Merged,
            created,
        });
    } else if pr.state == "CLOSED"
        && let Some(created) = pr.closed_at
    {
        out.push(TimelineEvent {
            actor: None,
            kind: EventKind::Declined,
            created,
        });
    }

    Ok(out)
}

fn review_kind(state: &str) -> Option<EventKind> {
    match state {
        "APPROVED" => Some(EventKind::Approved),
        "CHANGES_REQUESTED" => Some(EventKind::ChangesRequested),
        "DISMISSED" => Some(EventKind::ReviewRemoved),
        _ => None,
    }
}

const fn login_user(login: String) -> User {
    User { username: login }
}
