use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh_json;
use crate::domain::comment::{Comment, ReviewThread};
use crate::domain::user::User;

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
    path: String,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    original_line: Option<usize>,
    /// Set on replies. The id of the root comment in the same thread.
    #[serde(default)]
    in_reply_to_id: Option<u64>,
}

pub fn fetch_review_threads(pr_number: u64) -> Result<Vec<ReviewThread>, FetchError> {
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/comments");
    let comments: Vec<GhPrComment> = run_gh_json(&["api", "--paginate", &endpoint])?;

    let parent_map: HashMap<u64, Option<u64>> =
        comments.iter().map(|c| (c.id, c.in_reply_to_id)).collect();

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

        let domain_comments: Vec<Comment> = group
            .into_iter()
            .map(|gc| Comment {
                author: User {
                    username: gc.user.login,
                },
                content: gc.body,
                created: gc.created_at,
            })
            .collect();

        threads.push(ReviewThread {
            path,
            line,
            old_line: None,
            comments: domain_comments,
            resolved: false,
        });
    }

    threads.sort_by_key(|t| {
        t.comments
            .first()
            .map(|c| c.created)
            .unwrap_or_else(Utc::now)
    });

    Ok(threads)
}

fn find_root(id: u64, parent_map: &HashMap<u64, Option<u64>>) -> u64 {
    let mut current = id;
    for _ in 0..100 {
        match parent_map.get(&current) {
            Some(Some(parent)) => current = *parent,
            _ => return current,
        }
    }
    current
}
