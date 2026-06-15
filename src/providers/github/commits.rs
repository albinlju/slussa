use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::commit::Commit;
use crate::providers::error::FetchError;
use crate::providers::github::run_pr_graphql;

const QUERY: &str = "query($owner: String!, $name: String!, $pr: Int!) { \
  repository(owner: $owner, name: $name) { pullRequest(number: $pr) { \
    commits(first: 100) { nodes { commit { \
      oid messageHeadline authoredDate additions deletions author { name } \
    } } } } } }";

#[derive(Debug, Deserialize)]
struct GqlPullRequest {
    commits: GqlCommits,
}

#[derive(Debug, Deserialize)]
struct GqlCommits {
    nodes: Vec<GqlCommitNode>,
}

#[derive(Debug, Deserialize)]
struct GqlCommitNode {
    commit: GqlCommit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlCommit {
    oid: String,
    #[serde(default)]
    message_headline: String,
    authored_date: DateTime<Utc>,
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
    #[serde(default)]
    author: Option<GqlGitActor>,
}

#[derive(Debug, Deserialize)]
struct GqlGitActor {
    #[serde(default)]
    name: Option<String>,
}

pub fn fetch_commits(pr_number: u64) -> Result<Vec<Commit>, FetchError> {
    let pr: GqlPullRequest = run_pr_graphql(QUERY, pr_number)?;
    Ok(pr
        .commits
        .nodes
        .into_iter()
        .map(|n| map_commit(n.commit))
        .collect())
}

fn map_commit(c: GqlCommit) -> Commit {
    Commit {
        oid: c.oid,
        headline: c.message_headline,
        author_name: c.author.and_then(|a| a.name).unwrap_or_default(),
        authored_at: c.authored_date,
        additions: c.additions,
        deletions: c.deletions,
    }
}
