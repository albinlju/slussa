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
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use prs::fetch_prs;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::clients::error::FetchError;
use crate::domain::comment::{Comment, Reaction};
use crate::domain::user::User;

/// One `gh api graphql` call against the current repo's PR, returning the
/// `pullRequest` payload. `P` mirrors the fields the query selects.
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

/// The comment-node field selection matching [`GqlComment`]. Interpolated
/// into every query that selects comment nodes, so the two can't drift.
pub(super) const COMMENT_FIELDS: &str = "body createdAt author { login } \
    reactionGroups { content viewerHasReacted users { totalCount } }";

/// GraphQL comment node, shared by the issue-comments and review-threads
/// queries.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GqlComment {
    #[serde(default)]
    body: String,
    created_at: DateTime<Utc>,
    /// `null` for deleted accounts.
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
        author: User {
            username: c.author.map(|a| a.login).unwrap_or_default(),
        },
        content: c.body,
        created: c.created_at,
        reactions,
    }
}

/// GraphQL `reactionGroups` content → emoji. `None` for unknown values.
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
