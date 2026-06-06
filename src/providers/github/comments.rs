use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::process::Command;

use crate::domain::comment::Comment;
use crate::domain::user::User;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommentAuthor {
    #[serde(default)]
    login: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhComment {
    #[serde(default)]
    body: String,
    author: GhCommentAuthor,
    created_at: DateTime<Utc>,
    #[serde(default)]
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct GhCommentsResponse {
    comments: Vec<GhComment>,
}

pub fn fetch_comments(pr_number: u64) -> Vec<Comment> {
    let output = Command::new("gh")
        .args(["pr", "view", &pr_number.to_string(), "--json", "comments"])
        .output()
        .expect("gh not installed");

    // Don't panic on parse failure — gh schema varies between PR types and
    // we'd rather show an empty list than rip down the worker thread (which
    // smears the panic across the terminal).
    let resp: GhCommentsResponse = match serde_json::from_slice(&output.stdout) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    resp.comments
        .into_iter()
        .enumerate()
        .map(|(idx, gh)| map_comment(idx as u64, gh))
        .collect()
}

fn map_comment(idx: u64, gh: GhComment) -> Comment {
    let updated = gh.updated_at.unwrap_or(gh.created_at);
    Comment {
        id: idx,
        author: User {
            id: gh.author.login.clone(),
            username: gh.author.login,
            display_name: gh.author.name,
            avatar_url: None,
        },
        content: gh.body,
        created: gh.created_at,
        updated,
        replies: vec![],
        resolved: false,
    }
}
