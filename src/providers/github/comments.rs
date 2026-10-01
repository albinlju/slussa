use crate::domain::comment::{Comment, CommentKey, CommentKind};
use crate::providers::error::FetchError;
use crate::providers::github::{COMMENT_FIELDS, GqlComment, map_gql_comment};

pub fn fetch_comments(pr_number: u64) -> Result<Vec<Comment>, FetchError> {
    let nodes: Vec<GqlComment> =
        super::pagination::pr_nodes(pr_number, "comments", COMMENT_FIELDS)?;
    Ok(nodes
        .into_iter()
        .map(|c| Comment {
            reply_to: None,
            ..map_gql_comment(c)
        })
        .collect())
}

pub fn post_comment(
    pr_number: u64,
    path: &str,
    line: usize,
    removed: bool,
    body: &str,
    revision: &crate::domain::diff::DiffRevision,
) -> Result<(), FetchError> {
    let commit_id = &revision.head;
    let side = if removed { "LEFT" } else { "RIGHT" };
    super::cli::run_gh(&[
        "api",
        "--method",
        "POST",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/comments"),
        "-f",
        &format!("body={body}"),
        "-f",
        &format!("commit_id={commit_id}"),
        "-f",
        &format!("path={path}"),
        "-F",
        &format!("line={line}"),
        "-f",
        &format!("side={side}"),
    ])?;
    Ok(())
}

pub fn post_pr_comment(pr_number: u64, body: &str) -> Result<(), FetchError> {
    super::cli::run_gh(&[
        "api",
        "--method",
        "POST",
        &format!("repos/{{owner}}/{{repo}}/issues/{pr_number}/comments"),
        "-f",
        &format!("body={body}"),
    ])?;
    Ok(())
}

pub fn reply_comment(pr_number: u64, parent: u64, body: &str) -> Result<(), FetchError> {
    super::cli::run_gh(&[
        "api",
        "--method",
        "POST",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/comments/{parent}/replies"),
        "-f",
        &format!("body={body}"),
    ])?;
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

pub fn edit_comment(comment: CommentKey, body: &str) -> Result<(), FetchError> {
    super::cli::run_gh(&[
        "api",
        "--method",
        "PATCH",
        &comment_endpoint(comment),
        "-f",
        &format!("body={body}"),
    ])?;
    Ok(())
}

pub fn delete_comment(comment: CommentKey) -> Result<(), FetchError> {
    super::cli::run_gh(&["api", "--method", "DELETE", &comment_endpoint(comment)])?;
    Ok(())
}

pub fn set_thread_resolved(node_id: &str, resolved: bool) -> Result<(), FetchError> {
    let mutation = if resolved {
        "resolveReviewThread"
    } else {
        "unresolveReviewThread"
    };
    let query = format!(
        "mutation {{ {mutation}(input: {{ threadId: \"{node_id}\" }}) {{ thread {{ isResolved }} }} }}"
    );
    super::cli::run_gh(&["api", "graphql", "-f", &format!("query={query}")])?;
    Ok(())
}

pub(super) fn head_sha(pr_number: u64) -> Result<String, FetchError> {
    let out = super::cli::run_gh(&[
        "api",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
        "--jq",
        ".head.sha",
    ])?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

pub(super) fn diff_revision(
    pr_number: u64,
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
    let pr: Pr = super::cli::run_gh_json(&[
        "api",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
    ])?;
    Ok(crate::domain::diff::DiffRevision {
        head: pr.head.sha,
        base: Some(pr.base.sha),
        commit: false,
    })
}
