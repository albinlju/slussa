use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::event::{EventKind, TimelineEvent};
use crate::domain::user::User;

#[derive(Debug, Deserialize)]
struct PagedActivities {
    values: Vec<BbActivity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbActivity {
    action: String,
    #[serde(default)]
    created_date: i64,
    #[serde(default)]
    user: Option<BbUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbUser {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

pub fn fetch_events(config: &Config, pr_id: u64) -> Result<Vec<TimelineEvent>, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: PagedActivities = get_json(&config.repo.host, &path, &config.pat)?;
    let mut out: Vec<TimelineEvent> = Vec::new();
    for activity in page.values {
        let Some(kind) = map_action(&activity.action) else {
            continue;
        };
        out.push(TimelineEvent {
            actor: activity.user.map(map_user),
            kind,
            created: ms_to_utc(activity.created_date),
        });
    }
    Ok(out)
}

/// Bitbucket activity `action` → our [`EventKind`]. Returns `None` for actions
/// we don't surface in the timeline (COMMENTED lives in `fetch_comments`;
/// RESCOPED/UPDATED are noise).
fn map_action(action: &str) -> Option<EventKind> {
    match action {
        "OPENED" => Some(EventKind::Opened),
        "APPROVED" => Some(EventKind::Approved),
        "REVIEWED" => Some(EventKind::ChangesRequested),
        "UNAPPROVED" => Some(EventKind::ReviewRemoved),
        "MERGED" => Some(EventKind::Merged),
        "DECLINED" => Some(EventKind::Declined),
        "REOPENED" => Some(EventKind::Reopened),
        _ => None,
    }
}

fn map_user(u: BbUser) -> User {
    User {
        id: u.name.clone(),
        username: u.name,
        display_name: u.display_name,
        avatar_url: None,
    }
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
