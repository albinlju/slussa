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
    Ok(pr.comments.nodes.into_iter().map(map_gql_comment).collect())
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

fn head_sha(pr_number: u64) -> Result<String, FetchError> {
    let out = super::cli::run_gh(&[
        "api",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}"),
        "--jq",
        ".head.sha",
    ])?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}
