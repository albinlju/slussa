mod activities;
pub mod auth;
mod builds;
mod cli;
mod comments;
mod commits;
mod diff;
mod events;
mod graphql;
mod pagination;
mod prs;
mod review_threads;

pub use activities::fetch as fetch_activity;
pub use builds::{fetch_builds, rerun_failed};
pub use comments::{
    delete_comment, edit_comment, post_comment, post_pr_comment, reply_comment, set_thread_resolved,
};
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use prs::{fetch_info, fetch_pr, fetch_prs};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::domain::authorship::Authorship;
use crate::domain::comment::{Comment, CommentId, Reaction};
use crate::domain::pr::{DeletableBranch, MergeStrategy, Mergeability, PrId};
use crate::domain::review::{ReviewComment, ReviewVerdict};
use crate::domain::user::{AccountKind, User, Username};
use crate::providers::error::{FetchError, MergeError};

pub fn current_user() -> Result<Username, FetchError> {
    let out = cli::run_gh(&["api", "user", "--jq", ".login"])?;
    Username::parse(&String::from_utf8_lossy(&out))
        .ok_or_else(|| FetchError::ParseFailed("gh named no login".into()))
}

pub fn fetch_mergeability(pr_number: PrId) -> Result<Mergeability, FetchError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MergeFields {
        mergeable: String,
        #[serde(default)]
        merge_state_status: String,
        #[serde(default)]
        review_decision: Option<String>,
        #[serde(default)]
        auto_merge_request: Option<AutoMergeRequest>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AutoMergeRequest {
        merge_method: String,
    }
    let pr: MergeFields = run_pr_graphql(graphql::MERGEABILITY, pr_number)?;
    let status = merge_status(
        &pr.mergeable,
        &pr.merge_state_status,
        pr.review_decision.as_deref(),
    );
    let strategy = pr
        .auto_merge_request
        .and_then(|request| match request.merge_method.as_str() {
            "MERGE" => Some(MergeStrategy::Merge),
            "SQUASH" => Some(MergeStrategy::Squash),
            "REBASE" => Some(MergeStrategy::Rebase),
            _ => None,
        });
    Ok(match (strategy, status) {
        // A conflict is one more thing it waits for, and the request stays there
        // to be turned off.
        (Some(strategy), Mergeability::Blocked(waiting) | Mergeability::Conflicts(waiting)) => {
            Mergeability::AutoMerge { strategy, waiting }
        }
        (_, status) => status,
    })
}

/// Read GitHub's merge fields. GitHub computes them asynchronously, so a fresh
/// PR can answer `UNKNOWN` until it settles; a later refresh picks up the real
/// verdict. `BLOCKED` covers several rules and GitHub does not say which, so
/// the reason comes from the review decision when that explains it and is
/// otherwise general. An administrator may still be able to merge a `Blocked`
/// PR, which is why the state informs and does not forbid.
fn merge_status(mergeable: &str, state: &str, review_decision: Option<&str>) -> Mergeability {
    let blocked = |reason: &str| Mergeability::Blocked(vec![reason.to_owned()]);
    match (mergeable, state) {
        ("CONFLICTING", _) | (_, "DIRTY") => {
            Mergeability::Conflicts(vec!["It conflicts with the base branch.".into()])
        }
        (_, "DRAFT") => blocked("It is a draft."),
        (_, "BEHIND") => blocked("The branch is behind its base and must be updated."),
        (_, "BLOCKED") => blocked(match review_decision {
            Some("REVIEW_REQUIRED") => "An approving review is required.",
            Some("CHANGES_REQUESTED") => "A reviewer requested changes.",
            _ => "Required checks or branch rules are not satisfied.",
        }),
        // `UNSTABLE` means failing checks that are not required; GitHub still merges.
        ("MERGEABLE", "CLEAN" | "HAS_HOOKS" | "UNSTABLE") => Mergeability::Mergeable,
        _ => Mergeability::Unknown,
    }
}

/// Merge the PR, then delete the branch it came from when asked to. A branch
/// that cannot be deleted does not undo the merge: that is `BranchKept`.
pub fn merge(
    pr_number: PrId,
    strategy: MergeStrategy,
    delete: Option<&DeletableBranch>,
) -> Result<(), MergeError> {
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
    if let Some(branch) = delete {
        delete_branch(branch).map_err(MergeError::BranchKept)?;
    }
    Ok(())
}

fn delete_branch(branch: &DeletableBranch) -> Result<(), FetchError> {
    cli::run_gh(&[
        "api",
        "--method",
        "DELETE",
        &format!(
            "repos/{{owner}}/{{repo}}/git/refs/heads/{}",
            ref_path(branch.name())
        ),
    ])?;
    Ok(())
}

/// A branch name as the tail of a URL path: slashes stay, anything that is not
/// plain is escaped, so a `#` or `?` in a name does not end the path.
fn ref_path(name: &str) -> String {
    use std::fmt::Write;
    name.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
        out
    })
}

/// Merge the PR by itself with `strategy` once its checks and reviews allow it,
/// or stop it from doing so (`None`). GitHub refuses when the repository does
/// not allow auto-merge; that message reaches the user as it is.
pub fn set_auto_merge(pr_number: PrId, strategy: Option<MergeStrategy>) -> Result<(), FetchError> {
    let pr = pr_number.to_string();
    let flag = match strategy {
        Some(MergeStrategy::Merge) => "--merge",
        Some(MergeStrategy::Squash) => "--squash",
        Some(MergeStrategy::Rebase) => "--rebase",
        None => "--disable-auto",
    };
    let args: &[&str] = if strategy.is_some() {
        &["pr", "merge", &pr, "--auto", flag]
    } else {
        &["pr", "merge", &pr, flag]
    };
    cli::run_gh(args)?;
    Ok(())
}

/// Reopen a closed PR. GitHub refuses when the head branch is gone or the PR
/// was merged; that message reaches the user as it is.
pub fn reopen(pr_number: PrId) -> Result<(), FetchError> {
    cli::run_gh(&[
        "api",
        "--method",
        "PATCH",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
        "-f",
        "state=open",
    ])?;
    Ok(())
}

pub fn decline(pr_number: PrId) -> Result<(), FetchError> {
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
const fn review_event(verdict: ReviewVerdict) -> Option<&'static str> {
    match verdict {
        ReviewVerdict::Approve => Some("APPROVE"),
        ReviewVerdict::RequestChanges => Some("REQUEST_CHANGES"),
        ReviewVerdict::Comment => Some("COMMENT"),
        ReviewVerdict::Unapprove => None,
    }
}

pub fn submit_review(
    pr_number: PrId,
    verdict: ReviewVerdict,
    body: &str,
) -> Result<(), FetchError> {
    let Some(event) = review_event(verdict) else {
        return Err(FetchError::Unsupported(
            "This review verdict is not supported by GitHub.".into(),
        ));
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
    pr_number: PrId,
    verdict: ReviewVerdict,
    body: &str,
    comments: &[ReviewComment],
) -> Result<(), FetchError> {
    let Some(first) = comments.first() else {
        return submit_review(pr_number, verdict, body);
    };
    let Some(event) = review_event(verdict) else {
        return Err(FetchError::Unsupported(
            "This review verdict is not supported by GitHub.".into(),
        ));
    };
    let payload = review_payload(event, body, &first.revision, comments)?;
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/reviews");
    let input = serde_json::to_vec(&payload).map_err(|e| FetchError::ParseFailed(e.into()))?;
    cli::run_gh_stdin(
        &["api", "--method", "POST", &endpoint, "--input", "-"],
        &input,
    )?;
    Ok(())
}

/// A review on `revision`, which every comment in it has to be on.
fn review_payload(
    event: &str,
    body: &str,
    revision: &crate::domain::diff::DiffRevision,
    comments: &[ReviewComment],
) -> Result<serde_json::Value, FetchError> {
    if comments.iter().any(|c| &c.revision != revision) {
        return Err(FetchError::InvalidInput("A review must contain comments from one diff revision. Submit comments on different commits separately.".into()));
    }
    let commit_id = &revision.head;
    let comment_payload: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "path": c.path,
                "line": c.line.number(),
                "side": comments::side(c.line),
                "body": c.body,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "commit_id": commit_id,
        "event": event,
        "body": body,
        "comments": comment_payload,
    }))
}

pub(super) fn run_pr_graphql<P: DeserializeOwned>(
    query: &str,
    pr_number: PrId,
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
        &format!("query={}", graphql::compact(query)),
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

pub(super) const COMMENT_FIELDS: &str = "databaseId body createdAt author { login __typename } \
    reactionGroups { content viewerHasReacted users { totalCount } }";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GqlComment {
    #[serde(default)]
    commit: Option<GqlCommentCommit>,
    #[serde(default)]
    original_commit: Option<GqlCommentCommit>,
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
    /// `User`, `Bot`, `Organization`, `Mannequin`, ...
    #[serde(default, rename = "__typename")]
    typename: String,
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
    let id = c.database_id.map(CommentId);
    let account = match c.author.as_ref().map(|a| a.typename.as_str()) {
        Some("Bot") => AccountKind::Bot,
        // A foreign string: any other kind of account is a person's for this.
        Some(_) | None => AccountKind::Person,
    };
    Comment {
        id,
        author: User {
            username: c.author.map(|a| a.login).unwrap_or_default(),
        },
        account,
        authorship: Authorship::Human,
        content: c.body,
        created: c.created_at,
        reactions,
        // Review-thread comments reply via this databaseId; issue comments are
        // flat and get None overridden in fetch_comments.
        reply_to: id,
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

#[cfg(test)]
mod revision_tests {
    use super::*;
    #[test]
    fn review_uses_displayed_revision_and_rejects_mixed_revisions() {
        let first = ReviewComment {
            revision: crate::domain::diff::DiffRevision {
                head: "reviewed-sha".into(),
                base: None,
                commit: true,
            },
            path: "file.rs".into(),
            line: crate::domain::diff::LineRef::Old(7),
            body: "comment".into(),
        };
        let revision = first.revision.clone();
        let payload = review_payload(
            "COMMENT",
            "summary",
            &revision,
            std::slice::from_ref(&first),
        )
        .unwrap();
        assert_eq!(payload["commit_id"], "reviewed-sha");
        assert_eq!(payload["comments"][0]["side"], "LEFT");
        let mut other = first.clone();
        other.revision.head = "new-sha".into();
        assert!(review_payload("COMMENT", "summary", &revision, &[first, other]).is_err());
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;

    fn comment_by(author: &serde_json::Value) -> Comment {
        map_gql_comment(
            serde_json::from_value(serde_json::json!({
                "databaseId": 1, "body": "text", "createdAt": "2026-01-01T00:00:00Z",
                "author": author,
            }))
            .unwrap(),
        )
    }

    #[test]
    fn a_bot_account_is_a_bot_and_everything_else_is_a_person() {
        let kind = |author: serde_json::Value| comment_by(&author).account;
        assert_eq!(
            kind(serde_json::json!({"login": "coderabbitai", "__typename": "Bot"})),
            AccountKind::Bot
        );
        assert_eq!(
            kind(serde_json::json!({"login": "alice", "__typename": "User"})),
            AccountKind::Person
        );
        assert_eq!(
            kind(serde_json::json!({"login": "acme", "__typename": "Organization"})),
            AccountKind::Person
        );
        // An answer without the type (an older fixture, a deleted account).
        assert_eq!(
            kind(serde_json::json!({"login": "alice"})),
            AccountKind::Person
        );
        assert_eq!(kind(serde_json::Value::Null), AccountKind::Person);
    }

    #[test]
    fn the_comment_selection_asks_for_the_account_type() {
        assert!(COMMENT_FIELDS.contains("author { login __typename }"));
    }
}

#[derive(Debug, Deserialize)]
struct GqlCommentCommit {
    oid: String,
}
