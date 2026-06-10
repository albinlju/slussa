use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::comment::Comment;
use crate::domain::user::User;
use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh_json;

#[derive(Debug, Deserialize)]
struct GhCommentAuthor {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhComment {
    #[serde(default)]
    body: String,
    author: GhCommentAuthor,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GhCommentsResponse {
    comments: Vec<GhComment>,
}

pub fn fetch_comments(pr_number: u64) -> Result<Vec<Comment>, FetchError> {
    let pr_arg = pr_number.to_string();
    let resp: GhCommentsResponse =
        run_gh_json(&["pr", "view", &pr_arg, "--json", "comments"])?;
    Ok(resp.comments.into_iter().map(map_comment).collect())
}

fn map_comment(gh: GhComment) -> Comment {
    Comment {
        author: User {
            username: gh.author.login,
        },
        content: gh.body,
        created: gh.created_at,
    }
}
