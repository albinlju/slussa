use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::commit::Commit;
use crate::providers::error::FetchError;

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
    let nodes: Vec<GqlCommitNode> = super::pagination::pr_nodes(
        pr_number,
        "commits",
        "commit { oid messageHeadline authoredDate additions deletions author { name } }",
    )?;
    Ok(nodes.into_iter().map(|n| map_commit(n.commit)).collect())
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
