mod activities;
pub mod auth;
mod builds;
mod cli;
mod comments;
mod commits;
mod diff;
mod events;
mod prs;
mod review_threads;

pub use activities::fetch as fetch_activity;
pub use builds::fetch_builds;
pub use comments::{
    delete_comment, edit_comment, post_comment, post_pr_comment, reply_comment, set_thread_resolved,
};
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use prs::fetch_prs;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::domain::comment::{Comment, Reaction};
use crate::domain::user::User;
use crate::providers::error::FetchError;

pub fn current_user() -> Result<String, FetchError> {
    let out = cli::run_gh(&["api", "user", "--jq", ".login"])?;
    Ok(String::from_utf8_lossy(&out).trim().to_owned())
}

pub fn approve(pr_number: u64) -> Result<(), FetchError> {
    cli::run_gh(&[
        "api",
        "--method",
        "POST",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/reviews"),
        "-f",
        "event=APPROVE",
    ])?;
    Ok(())
}

pub(super) fn run_pr_graphql<P: DeserializeOwned>(
    query: &str,
    pr_number: u64,
) -> Result<P, FetchError> {
    let resp: GqlResponse<P> = cli::run_gh_json(&[
        "api",
        "graphql",
        "-F",
        "owner={owner}",
        "-F",
        "name={repo}",
        "-F",
        &format!("pr={pr_number}"),
        "-f",
        &format!("query={query}"),
    ])?;
    Ok(resp.data.repository.pull_request)
}

#[derive(Debug, Deserialize)]
struct GqlResponse<P> {
    data: GqlData<P>,
}

#[derive(Debug, Deserialize)]
struct GqlData<P> {
    repository: GqlRepository<P>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRepository<P> {
    pull_request: P,
}

pub(super) const COMMENT_FIELDS: &str = "databaseId body createdAt author { login } \
    reactionGroups { content viewerHasReacted users { totalCount } }";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GqlComment {
    #[serde(default)]
    database_id: Option<u64>,
    #[serde(default)]
    body: String,
    created_at: DateTime<Utc>,
    author: Option<GqlAuthor>,
    #[serde(default)]
    reaction_groups: Vec<GqlReactionGroup>,
}

#[derive(Debug, Deserialize)]
struct GqlAuthor {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlReactionGroup {
    #[serde(default)]
    content: String,
    #[serde(default)]
    viewer_has_reacted: bool,
    #[serde(default)]
    users: GqlReactionUsers,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlReactionUsers {
    #[serde(default)]
    total_count: u32,
}

pub(super) fn map_gql_comment(c: GqlComment) -> Comment {
    let reactions = c
        .reaction_groups
        .iter()
        .filter(|g| g.users.total_count > 0)
        .filter_map(|g| {
            reaction_emoji(&g.content).map(|emoji| Reaction {
                emoji: emoji.to_string(),
                count: g.users.total_count,
                mine: g.viewer_has_reacted,
            })
        })
        .collect();
    Comment {
        id: c.database_id,
        author: User {
            username: c.author.map(|a| a.login).unwrap_or_default(),
        },
        content: c.body,
        created: c.created_at,
        reactions,
        // Review-thread comments reply via this databaseId; issue comments are
        // flat and get None overridden in fetch_comments.
        reply_to: c.database_id,
    }
}

fn reaction_emoji(name: &str) -> Option<&'static str> {
    Some(match name {
        "THUMBS_UP" => "👍",
        "THUMBS_DOWN" => "👎",
        "LAUGH" => "😄",
        "HOORAY" => "🎉",
        "CONFUSED" => "😕",
        "HEART" => "❤",
        "ROCKET" => "🚀",
        "EYES" => "👀",
        _ => return None,
    })
}
