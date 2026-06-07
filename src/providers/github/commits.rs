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

    let resp: GhCommitsResponse = match serde_json::from_slice(&output.stdout) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut commits: Vec<Commit> = resp.commits.into_iter().map(map_commit).collect();

    // `gh pr view --json commits` doesn't surface per-commit diff stats, so we
    // fan out and fetch each commit's stats from the REST endpoint in parallel
    // threads. The work is IO-bound (subprocess + network), so std::thread is
    // simpler than introducing rayon for a handful of commits.
    let handles: Vec<_> = commits
        .iter()
        .map(|c| {
            let sha = c.oid.clone();
            std::thread::spawn(move || fetch_commit_stats(&sha))
        })
        .collect();

    for (commit, handle) in commits.iter_mut().zip(handles) {
        if let Ok(Some((adds, dels))) = handle.join() {
            commit.additions = adds;
            commit.deletions = dels;
        }
    }

    commits
}

#[derive(Debug, Deserialize)]
struct GhCommitDetail {
    stats: GhStats,
}

#[derive(Debug, Deserialize)]
struct GhStats {
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
}

fn fetch_commit_stats(sha: &str) -> Option<(u32, u32)> {
    let output = Command::new("gh")
        .args(["api", &format!("repos/{{owner}}/{{repo}}/commits/{}", sha)])
        .output()
        .ok()?;

    let resp: GhCommitDetail = serde_json::from_slice(&output.stdout).ok()?;
    Some((resp.stats.additions, resp.stats.deletions))
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
        additions: 0,
        deletions: 0,
    }
}
