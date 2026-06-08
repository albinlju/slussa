use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::commit::Commit;

#[derive(Debug, Deserialize)]
struct PagedCommits {
    values: Vec<BbCommit>,
}

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

pub fn fetch_commits(config: &Config, pr_id: u64) -> Result<Vec<Commit>, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/commits?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: PagedCommits = get_json(&config.repo.host, &path, &config.pat)?;
    Ok(page.values.into_iter().map(map_commit).collect())
}

fn map_commit(c: BbCommit) -> Commit {
    let (headline, body) = split_message(&c.message);
    Commit {
        oid: c.id,
        headline,
        body,
        author_name: c.author.display_name.unwrap_or(c.author.name),
        authored_at: ms_to_utc(c.author_timestamp),
        additions: 0,
        deletions: 0,
    }
}

fn split_message(msg: &str) -> (String, String) {
    let mut lines = msg.lines();
    let headline = lines.next().unwrap_or("").to_string();
    let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    (headline, body)
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
