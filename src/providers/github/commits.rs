use super::GhRepo;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::pr::PrId;
use crate::domain::{
    authorship::Authorship,
    commit::{Commit, CommitOid},
    user::AccountKind,
};
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
    #[serde(default)]
    message: String,
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
    #[serde(default)]
    email: Option<String>,
}

/// GitHub gives a GitHub App's commits an address like
/// `123+claude[bot]@users.noreply.github.com`. The commit's author is a git
/// actor and not an account, so that address is the one sign of a bot.
fn is_bot_address(email: &str) -> bool {
    email.ends_with("[bot]@users.noreply.github.com")
}

pub fn fetch_commits(repo: &GhRepo, pr_number: PrId) -> Result<Vec<Commit>, FetchError> {
    let nodes: Vec<GqlCommitNode> = super::pagination::pr_nodes(
        repo,
        pr_number,
        "commits",
        "commit { oid messageHeadline message authoredDate additions deletions author { name email } }",
    )?;
    Ok(nodes.into_iter().map(|n| map_commit(n.commit)).collect())
}

fn map_commit(c: GqlCommit) -> Commit {
    let bot = c
        .author
        .as_ref()
        .and_then(|a| a.email.as_deref())
        .is_some_and(is_bot_address);
    Commit {
        oid: CommitOid(c.oid),
        headline: c.message_headline,
        message: c.message,
        account: if bot {
            AccountKind::Bot
        } else {
            AccountKind::Person
        },
        authorship: Authorship::Human,
        author_name: c.author.and_then(|a| a.name).unwrap_or_default(),
        authored_at: c.authored_date,
        additions: c.additions,
        deletions: c.deletions,
    }
}
