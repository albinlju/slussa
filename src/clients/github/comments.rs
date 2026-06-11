use serde::Deserialize;

use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh_json;
use crate::clients::github::{GqlComment, map_gql_comment};
use crate::domain::comment::Comment;

// GraphQL instead of `gh pr view --json comments`: only the GraphQL
// reactionGroups carry `viewerHasReacted`, which marks the user's own
// reactions in the UI.
const QUERY: &str = "query($owner: String!, $name: String!, $pr: Int!) { \
  repository(owner: $owner, name: $name) { pullRequest(number: $pr) { \
    comments(first: 100) { nodes { \
      body createdAt author { login } \
      reactionGroups { content viewerHasReacted users { totalCount } } \
    } } } } }";

#[derive(Debug, Deserialize)]
struct GqlResponse {
    data: GqlData,
}

#[derive(Debug, Deserialize)]
struct GqlData {
    repository: GqlRepository,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRepository {
    pull_request: GqlPullRequest,
}

#[derive(Debug, Deserialize)]
struct GqlPullRequest {
    comments: GqlComments,
}

#[derive(Debug, Deserialize)]
struct GqlComments {
    nodes: Vec<GqlComment>,
}

pub fn fetch_comments(pr_number: u64) -> Result<Vec<Comment>, FetchError> {
    let resp: GqlResponse = run_gh_json(&[
        "api",
        "graphql",
        "-F",
        "owner={owner}",
        "-F",
        "name={repo}",
        "-F",
        &format!("pr={pr_number}"),
        "-f",
        &format!("query={QUERY}"),
    ])?;
    Ok(resp
        .data
        .repository
        .pull_request
        .comments
        .nodes
        .into_iter()
        .map(map_gql_comment)
        .collect())
}
