//! Bitbucket Data Center exposes PR comments through the `/activities` feed —
//! each activity is one event (commented, approved, merged, ...). We walk it
//! and keep only the `COMMENT` activities for general (non-inline) comments.

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::{comment::Comment, user::User};

#[derive(Debug, Deserialize)]
struct PagedActivities {
    values: Vec<BbActivity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbActivity {
    action: String,
    #[serde(default)]
    comment_anchor: Option<serde_json::Value>,
    #[serde(default)]
    comment: Option<BbComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbComment {
    id: u64,
    author: BbUser,
    text: String,
    created_date: i64,
    updated_date: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbUser {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

pub fn fetch_comments(config: &Config, pr_id: u64) -> Result<Vec<Comment>, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: PagedActivities = get_json(&config.repo.host, &path, &config.pat)?;
    let mut out: Vec<Comment> = Vec::new();
    for activity in page.values {
        // Skip non-comment activities. Inline review comments (with anchor)
        // belong in `fetch_review_threads`, not here.
        if activity.action != "COMMENTED" || activity.comment_anchor.is_some() {
            continue;
        }
        if let Some(c) = activity.comment {
            out.push(map_comment(c));
        }
    }
    Ok(out)
}

fn map_comment(c: BbComment) -> Comment {
    Comment {
        id: c.id,
        author: User {
            id: c.author.name.clone(),
            username: c.author.name,
            display_name: c.author.display_name,
            avatar_url: None,
        },
        content: c.text,
        created: ms_to_utc(c.created_date),
        updated: ms_to_utc(c.updated_date),
        replies: Vec::new(),
        resolved: false,
    }
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
