use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh_json;
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
    authored_date: DateTime<Utc>,
    #[serde(default)]
    authors: Vec<GhCommitAuthor>,
}

#[derive(Debug, Deserialize)]
struct GhCommitsResponse {
    commits: Vec<GhCommit>,
}

pub fn fetch_commits(pr_number: u64) -> Result<Vec<Commit>, FetchError> {
    let pr_arg = pr_number.to_string();
    let resp: GhCommitsResponse = run_gh_json(&["pr", "view", &pr_arg, "--json", "commits"])?;

    let mut commits: Vec<Commit> = resp.commits.into_iter().map(map_commit).collect();

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

    Ok(commits)
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
    let resp: GhCommitDetail =
        run_gh_json(&["api", &format!("repos/{{owner}}/{{repo}}/commits/{sha}")]).ok()?;
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
        author_name,
        authored_at: gh.authored_date,
        additions: 0,
        deletions: 0,
    }
}
