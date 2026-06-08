use std::collections::HashSet;

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::{Config, structured};
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::{
    comment::{Comment, ReviewThread},
    user::User,
};

pub fn fetch_review_threads(config: &Config, pr_id: u64) -> Result<Vec<ReviewThread>, FetchError> {
    let mut threads = structured::fetch(config, pr_id)?.threads;

    let mut seen: HashSet<u64> = threads
        .iter()
        .filter_map(|t| t.comments.first().map(|c| c.id))
        .collect();

    for thread in anchored_activity_threads(config, pr_id)? {
        if let Some(first) = thread.comments.first()
            && seen.insert(first.id)
        {
            threads.push(thread);
        }
    }

    Ok(threads)
}

#[derive(Debug, Deserialize)]
struct PagedActivities {
    values: Vec<BbActivity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbActivity {
    action: String,
    #[serde(default)]
    comment_anchor: Option<BbAnchor>,
    #[serde(default)]
    comment: Option<BbComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbAnchor {
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: usize,
    #[serde(default)]
    line_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbComment {
    id: u64,
    author: BbUser,
    text: String,
    created_date: i64,
    updated_date: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    comments: Vec<BbComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbUser {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

/// Parse the anchored (inline) comments out of the activities feed into review
/// threads. No diff snippet — just the file/line anchor and the comment text.
fn anchored_activity_threads(config: &Config, pr_id: u64) -> Result<Vec<ReviewThread>, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: PagedActivities = get_json(&config.repo.host, &path, &config.pat)?;

    let mut out: Vec<ReviewThread> = Vec::new();
    for activity in page.values {
        if activity.action != "COMMENTED" {
            continue;
        }
        let (Some(anchor), Some(root)) = (activity.comment_anchor, activity.comment) else {
            continue;
        };
        // Removed lines anchor to the old-file line; added/context to the new.
        let (line, old_line) = if anchor.line_type.eq_ignore_ascii_case("REMOVED") {
            (None, Some(anchor.line))
        } else {
            (Some(anchor.line), None)
        };
        let mut flat: Vec<Comment> = Vec::new();
        collect_replies(&root, &mut flat);
        out.push(ReviewThread {
            path: anchor.path,
            line,
            old_line,
            diff_hunk: String::new(),
            resolved: root.state.eq_ignore_ascii_case("RESOLVED"),
            comments: flat,
        });
    }
    Ok(out)
}

fn collect_replies(c: &BbComment, out: &mut Vec<Comment>) {
    out.push(map_comment(c));
    for child in &c.comments {
        collect_replies(child, out);
    }
}

fn map_comment(c: &BbComment) -> Comment {
    Comment {
        id: c.id,
        author: User {
            id: c.author.name.clone(),
            username: c.author.name.clone(),
            display_name: c.author.display_name.clone(),
            avatar_url: None,
        },
        content: c.text.clone(),
        created: ms_to_utc(c.created_date),
        updated: ms_to_utc(c.updated_date),
        replies: Vec::new(),
        resolved: c.state.eq_ignore_ascii_case("RESOLVED"),
    }
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
