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
use crate::domain::pr::{Mergeability, MergeStrategy};
use crate::domain::review::{ReviewComment, ReviewVerdict};
use crate::domain::user::User;
use crate::providers::error::FetchError;

pub fn current_user() -> Result<String, FetchError> {
    let out = cli::run_gh(&["api", "user", "--jq", ".login"])?;
    Ok(String::from_utf8_lossy(&out).trim().to_owned())
}

pub fn fetch_mergeability(pr_number: u64) -> Result<Mergeability, FetchError> {
    #[derive(Deserialize)]
    struct Mergeable {
        mergeable: String,
    }
    let query = "query($owner: String!, $name: String!, $pr: Int!) { \
        repository(owner: $owner, name: $name) { \
          pullRequest(number: $pr) { mergeable } } }";
    let pr: Mergeable = run_pr_graphql(query, pr_number)?;
    // GitHub computes this asynchronously, so a fresh PR can answer UNKNOWN
    // until it settles — a later refresh picks up the real verdict.
    Ok(match pr.mergeable.as_str() {
        "MERGEABLE" => Mergeability::Mergeable,
        "CONFLICTING" => Mergeability::Conflicts,
        _ => Mergeability::Unknown,
    })
}

pub fn merge(pr_number: u64, strategy: MergeStrategy) -> Result<(), FetchError> {
    let method = match strategy {
        MergeStrategy::Merge => "merge",
        MergeStrategy::Squash => "squash",
        MergeStrategy::Rebase => "rebase",
    };
    cli::run_gh(&[
        "api",
        "--method",
        "PUT",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/merge"),
        "-f",
        &format!("merge_method={method}"),
    ])?;
    Ok(())
}

pub fn decline(pr_number: u64) -> Result<(), FetchError> {
    // GitHub has no "decline" — closing the PR is the equivalent.
    cli::run_gh(&[
        "api",
        "--method",
        "PATCH",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
        "-f",
        "state=closed",
    ])?;
    Ok(())
}

/// GitHub review event for a verdict, or `None` where it has no GitHub
/// equivalent (`Unapprove` — reviews are immutable, so there's no "undo").
fn review_event(verdict: ReviewVerdict) -> Option<&'static str> {
    match verdict {
        ReviewVerdict::Approve => Some("APPROVE"),
        ReviewVerdict::RequestChanges => Some("REQUEST_CHANGES"),
        ReviewVerdict::Comment => Some("COMMENT"),
        ReviewVerdict::Unapprove => None,
    }
}

pub fn submit_review(pr_number: u64, verdict: ReviewVerdict, body: &str) -> Result<(), FetchError> {
    let Some(event) = review_event(verdict) else {
        return Ok(());
    };
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/reviews");
    let event_arg = format!("event={event}");
    let body_arg = format!("body={body}");
    let mut args: Vec<&str> = vec!["api", "--method", "POST", &endpoint, "-f", &event_arg];
    if !body.is_empty() {
        args.push("-f");
        args.push(&body_arg);
    }
    cli::run_gh(&args)?;
    Ok(())
}

/// Submit a verdict with a batch of inline `comments` in one atomic call — the
/// reviews endpoint takes a `comments` array, fed as JSON on stdin since `-f`
/// flags can't express it.
pub fn submit_full_review(
    pr_number: u64,
    verdict: ReviewVerdict,
    body: &str,
    comments: &[ReviewComment],
) -> Result<(), FetchError> {
    if comments.is_empty() {
        return submit_review(pr_number, verdict, body);
    }
    let Some(event) = review_event(verdict) else {
        return Ok(());
    };
    let commit_id = comments::head_sha(pr_number)?;
    let comment_payload: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "path": c.path,
                "line": c.line,
                "side": if c.removed { "LEFT" } else { "RIGHT" },
                "body": c.body,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "commit_id": commit_id,
        "event": event,
        "body": body,
        "comments": comment_payload,
    });
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/reviews");
    let input = serde_json::to_vec(&payload).map_err(|e| FetchError::ParseFailed(e.to_string()))?;
    cli::run_gh_stdin(&["api", "--method", "POST", &endpoint, "--input", "-"], &input)?;
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
