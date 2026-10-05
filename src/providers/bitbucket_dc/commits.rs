use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::domain::pr::PrId;
use crate::domain::{
    authorship::Authorship,
    commit::{Commit, CommitOid},
    user::AccountKind,
};
use crate::providers::bitbucket_dc::http::get_all;
use crate::providers::error::FetchError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbCommit {
    id: String,
    #[serde(default)]
    message: String,
    author: BbAuthor,
    author_timestamp: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbAuthor {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

pub fn fetch_commits(config: &Config, pr_id: PrId) -> Result<Vec<Commit>, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/commits?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let values: Vec<BbCommit> = get_all(&config.repo.base_url, &path, &config.pat)?;
    values.into_iter().map(map_commit).collect()
}

fn map_commit(c: BbCommit) -> Result<Commit, FetchError> {
    let oid = CommitOid::parse(&c.id)
        .ok_or_else(|| FetchError::ParseFailed("a commit id that is not hexadecimal".into()))?;
    Ok(Commit {
        oid,
        headline: c.message.lines().next().unwrap_or("").to_string(),
        message: c.message,
        // Bitbucket Data Center does not mark bot accounts.
        account: AccountKind::Person,
        authorship: Authorship::Human,
        author_name: c.author.display_name.unwrap_or(c.author.name),
        authored_at: ms_to_utc(c.author_timestamp),
        additions: 0,
        deletions: 0,
    })
}
