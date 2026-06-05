use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::process::Command;

use crate::domain::commit::Commit;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommitAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommit {
    oid: String,
    #[serde(default)]
    message_headline: String,
    #[serde(default)]
    message_body: String,
    authored_date: DateTime<Utc>,
    #[serde(default)]
    authors: Vec<GhCommitAuthor>,
}

#[derive(Debug, Deserialize)]
struct GhCommitsResponse {
    commits: Vec<GhCommit>,
}

pub fn fetch_commits(pr_number: u64) -> Vec<Commit> {
    let output = Command::new("gh")
        .args(["pr", "view", &pr_number.to_string(), "--json", "commits"])
        .output()
        .expect("gh not installed");

    let resp: GhCommitsResponse =
        serde_json::from_slice(&output.stdout).expect("kunde inte parsa commits");

    resp.commits.into_iter().map(map_commit).collect()
}

fn map_commit(gh: GhCommit) -> Commit {
    let author_name = gh
        .authors
        .into_iter()
        .next()
        .map(|a| a.name)
        .unwrap_or_default();

    Commit {
        oid: gh.oid,
        headline: gh.message_headline,
        body: gh.message_body,
        author_name,
        authored_at: gh.authored_date,
    }
}
