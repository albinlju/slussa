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
