use super::GhRepo;
use crate::domain::{
    comment::{Comment, CommentId, CommentKey, CommentKind},
    diff::LineRef,
    pr::PrId,
    review::ReviewComment,
};
use crate::providers::error::FetchError;
use crate::providers::github::{COMMENT_FIELDS, GqlComment, map_gql_comment};

pub fn fetch_comments(repo: &GhRepo, pr_number: PrId) -> Result<Vec<Comment>, FetchError> {
    let nodes: Vec<GqlComment> =
        super::pagination::pr_nodes(repo, pr_number, "comments", COMMENT_FIELDS)?;
    Ok(nodes
        .into_iter()
        .map(|c| Comment {
            reply_to: None,
            ..map_gql_comment(c)
        })
        .collect())
}

/// GitHub's name for the side of the diff a line is on.
pub(super) const fn side(line: LineRef) -> &'static str {
    match line {
        LineRef::Old(_) => "LEFT",
        LineRef::New(_) => "RIGHT",
    }
}

pub fn post_comment(
    repo: &GhRepo,
    pr_number: PrId,
    comment: &ReviewComment,
) -> Result<(), FetchError> {
    let ReviewComment {
        revision,
        path,
        line,
        body,
    } = comment;
    super::cli::send_json(
        repo,
        "POST",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/comments"),
        &serde_json::json!({
            "body": body,
            "commit_id": revision.head,
            "path": path,
            "line": line.number(),
            "side": side(*line),
        }),
    )?;
    Ok(())
}

pub fn post_pr_comment(repo: &GhRepo, pr_number: PrId, body: &str) -> Result<(), FetchError> {
    super::cli::send_json(
        repo,
        "POST",
        &format!("repos/{{owner}}/{{repo}}/issues/{pr_number}/comments"),
        &serde_json::json!({ "body": body }),
    )?;
    Ok(())
}

pub fn reply_comment(
    repo: &GhRepo,
    pr_number: PrId,
    parent: CommentId,
    body: &str,
) -> Result<(), FetchError> {
    super::cli::send_json(
        repo,
        "POST",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/comments/{parent}/replies"),
        &serde_json::json!({ "body": body }),
    )?;
    Ok(())
}

/// Review comments live under `pulls`, comments on the PR under `issues`.
fn comment_endpoint(comment: CommentKey) -> String {
    let collection = match comment.kind {
        CommentKind::Review => "pulls",
        CommentKind::Conversation => "issues",
    };
    format!(
        "repos/{{owner}}/{{repo}}/{collection}/comments/{}",
        comment.id
    )
}

pub fn edit_comment(repo: &GhRepo, comment: CommentKey, body: &str) -> Result<(), FetchError> {
    super::cli::send_json(
        repo,
        "PATCH",
        &comment_endpoint(comment),
        &serde_json::json!({ "body": body }),
    )?;
    Ok(())
}

pub fn delete_comment(repo: &GhRepo, comment: CommentKey) -> Result<(), FetchError> {
    super::cli::run_gh(
        repo,
        &["api", "--method", "DELETE", &comment_endpoint(comment)],
    )?;
    Ok(())
}

pub fn set_thread_resolved(repo: &GhRepo, node_id: &str, resolved: bool) -> Result<(), FetchError> {
    let mutation = if resolved {
        "resolveReviewThread"
    } else {
        "unresolveReviewThread"
    };
    let query = format!(
        "mutation {{ {mutation}(input: {{ threadId: \"{node_id}\" }}) {{ thread {{ isResolved }} }} }}"
    );
    super::cli::run_gh(repo, &["api", "graphql", "-f", &format!("query={query}")])?;
    Ok(())
}

pub(super) fn head_sha(repo: &GhRepo, pr_number: PrId) -> Result<String, FetchError> {
    let out = super::cli::run_gh(
        repo,
        &[
            "api",
            &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
            "--jq",
            ".head.sha",
        ],
    )?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Fails unless the PR's branch is still at `head`, the one that was read.
pub(super) fn ensure_head(
    repo: &GhRepo,
    pr_number: PrId,
    head: &crate::domain::review::ReviewedHead,
) -> Result<(), FetchError> {
    if head_sha(repo, pr_number)? == head.as_str() {
        Ok(())
    } else {
        Err(super::merge::moved_since_read())
    }
}

pub(super) fn diff_revision(
    repo: &GhRepo,
    pr_number: PrId,
) -> Result<crate::domain::diff::DiffRevision, FetchError> {
    #[derive(serde::Deserialize)]
    struct Ref {
        sha: String,
    }
    #[derive(serde::Deserialize)]
    struct Pr {
        head: Ref,
        base: Ref,
    }
    let pr: Pr = super::cli::run_gh_json(
        repo,
        &[
            "api",
            &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
        ],
    )?;
    Ok(crate::domain::diff::DiffRevision {
        head: pr.head.sha,
        base: Some(pr.base.sha),
        commit: false,
    })
}
