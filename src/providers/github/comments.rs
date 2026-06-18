use serde::Deserialize;

use crate::domain::comment::Comment;
use crate::providers::error::FetchError;
use crate::providers::github::{COMMENT_FIELDS, GqlComment, map_gql_comment, run_pr_graphql};

#[derive(Debug, Deserialize)]
struct GqlPullRequest {
    comments: GqlComments,
}

#[derive(Debug, Deserialize)]
struct GqlComments {
    nodes: Vec<GqlComment>,
}

pub fn fetch_comments(pr_number: u64) -> Result<Vec<Comment>, FetchError> {
    let query = format!(
        "query($owner: String!, $name: String!, $pr: Int!) {{ \
           repository(owner: $owner, name: $name) {{ pullRequest(number: $pr) {{ \
             comments(first: 100) {{ nodes {{ {COMMENT_FIELDS} }} }} }} }} }}"
    );
    let pr: GqlPullRequest = run_pr_graphql(&query, pr_number)?;
    // Issue comments are flat: a reply is just another top-level PR comment.
    Ok(pr
        .comments
        .nodes
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
) -> Result<(), FetchError> {
    let commit_id = head_sha(pr_number)?;
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

pub fn edit_comment(comment_id: u64, review: bool, body: &str) -> Result<(), FetchError> {
    // Review (line) comments live under `pulls`, PR-level ones under `issues`.
    let kind = if review { "pulls" } else { "issues" };
    super::cli::run_gh(&[
        "api",
        "--method",
        "PATCH",
        &format!("repos/{{owner}}/{{repo}}/{kind}/comments/{comment_id}"),
        "-f",
        &format!("body={body}"),
    ])?;
    Ok(())
}

pub fn delete_comment(comment_id: u64, review: bool) -> Result<(), FetchError> {
    let kind = if review { "pulls" } else { "issues" };
    super::cli::run_gh(&[
        "api",
        "--method",
        "DELETE",
        &format!("repos/{{owner}}/{{repo}}/{kind}/comments/{comment_id}"),
    ])?;
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

fn head_sha(pr_number: u64) -> Result<String, FetchError> {
    let out = super::cli::run_gh(&[
        "api",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
        "--jq",
        ".head.sha",
    ])?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}
