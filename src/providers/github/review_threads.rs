use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::comment::{Comment, ReviewThread};
use crate::domain::user::User;
use crate::providers::github::error::{FetchError, run_gh};

#[derive(Debug, Deserialize)]
struct GhUser {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Deserialize)]
struct GhPrComment {
    id: u64,
    #[serde(default)]
    body: String,
    user: GhUser,
    created_at: DateTime<Utc>,
    #[serde(default)]
    updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    path: String,
    #[serde(default)]
    diff_hunk: String,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    original_line: Option<usize>,
    /// Set on replies. The id of the root comment in the same thread.
    #[serde(default)]
    in_reply_to_id: Option<u64>,
}

/// Fetch inline review comments via the REST API. `gh pr view --json reviews`
/// only returns the review-level bodies and the inline comments aren't fully
/// surfaced. The REST endpoint gives us each comment with its path, line, and
/// diff_hunk anchor — and `in_reply_to_id` lets us group replies back into
/// threads ourselves.
///
/// The `{owner}` and `{repo}` placeholders are substituted by gh automatically
/// from the current repo context.
pub fn fetch_review_threads(pr_number: u64) -> Result<Vec<ReviewThread>, FetchError> {
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{}/comments", pr_number);
    let stdout = run_gh(&["api", "--paginate", &endpoint])?;
    let comments: Vec<GhPrComment> =
        serde_json::from_slice(&stdout).map_err(|e| FetchError::ParseFailed(e.to_string()))?;

    // Map each comment to its in_reply_to_id so we can chase to the thread root.
    let parent_map: HashMap<u64, Option<u64>> =
        comments.iter().map(|c| (c.id, c.in_reply_to_id)).collect();

    // Group comments by the root id of their thread.
    let mut groups: HashMap<u64, Vec<GhPrComment>> = HashMap::new();
    for comment in comments {
        let root = find_root(comment.id, &parent_map);
        groups.entry(root).or_default().push(comment);
    }

    let mut threads: Vec<ReviewThread> = Vec::new();
    for (_, mut group) in groups {
        group.sort_by_key(|c| c.created_at);
        let Some(first) = group.first() else {
            continue;
        };
        if first.path.is_empty() {
            continue;
        }

        let path = first.path.clone();
        let line = first.line.or(first.original_line);
        let diff_hunk = first.diff_hunk.clone();

        let domain_comments: Vec<Comment> = group
            .into_iter()
            .enumerate()
            .map(|(i, gc)| Comment {
                id: i as u64,
                author: User {
                    id: gc.user.login.clone(),
                    username: gc.user.login,
                    display_name: None,
                    avatar_url: None,
                },
                content: gc.body,
                created: gc.created_at,
                updated: gc.updated_at.unwrap_or(gc.created_at),
                replies: vec![],
                resolved: false,
            })
            .collect();

        threads.push(ReviewThread {
            path,
            line,
            diff_hunk,
            comments: domain_comments,
            resolved: false,
        });
    }

    // Sort threads chronologically by their root comment so Overview shows them
    // in the order they were posted.
    threads.sort_by_key(|t| t.comments.first().map(|c| c.created).unwrap_or_else(Utc::now));

    Ok(threads)
}

fn find_root(id: u64, parent_map: &HashMap<u64, Option<u64>>) -> u64 {
    let mut current = id;
    // Cap the chase in case the data has a weird cycle. 100 is way past any
    // realistic reply depth.
    for _ in 0..100 {
        match parent_map.get(&current) {
            Some(Some(parent)) => current = *parent,
            _ => return current,
        }
    }
    current
}
