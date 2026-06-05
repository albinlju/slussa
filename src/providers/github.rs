use crate::domain::ci::{CiState, CiStatus};
use crate::domain::commit::Commit;
use crate::domain::diff::{Diff, DiffLine, FileDiff, Hunk};
use crate::domain::pr::{PrStatus, PullRequest};
use crate::domain::provider::ProviderKind;
use crate::domain::repo::Repo;
use crate::domain::user::User;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct GhAuthor {
    #[serde(default)]
    login: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPr {
    number: u64,
    title: String,
    #[serde(default)]
    body: Option<String>,
    author: GhAuthor,
    state: String,
    #[serde(default)]
    is_draft: bool,
    head_ref_name: String,
    base_ref_name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub fn fetch_prs() -> Vec<PullRequest> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--json",
            "title,number,author,state,isDraft,headRefName,baseRefName,body,createdAt,updatedAt",
        ])
        .output()
        .expect("gh not installed");

    let gh_prs: Vec<GhPr> =
        serde_json::from_slice(&output.stdout).expect("kunde inte parsa gh-output");

    gh_prs.into_iter().map(map_pr).collect()
}

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

pub fn fetch_diff(pr_number: u64) -> Diff {
    let output = Command::new("gh")
        .args(["pr", "diff", &pr_number.to_string()])
        .output()
        .expect("gh not installed");

    let text = String::from_utf8_lossy(&output.stdout);
    parse_unified_diff(&text)
}

fn parse_unified_diff(text: &str) -> Diff {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(hunk) = current_hunk.take() {
                if let Some(f) = current_file.as_mut() {
                    f.hunks.push(hunk);
                }
            }
            if let Some(f) = current_file.take() {
                files.push(f);
            }
            let path = rest
                .split_whitespace()
                .next()
                .map(|s| s.strip_prefix("a/").unwrap_or(s).to_string())
                .unwrap_or_default();
            current_file = Some(FileDiff {
                path,
                hunks: Vec::new(),
            });
        } else if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                if let Some(f) = current_file.as_mut() {
                    f.hunks.push(hunk);
                }
            }
            let (old_start, new_start) = parse_hunk_header(line);
            current_hunk = Some(Hunk {
                old_start,
                new_start,
                lines: Vec::new(),
            });
        } else if let Some(hunk) = current_hunk.as_mut() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine::Added(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine::Removed(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine::Context(rest.to_string()));
            }
        }
    }

    if let Some(hunk) = current_hunk.take() {
        if let Some(f) = current_file.as_mut() {
            f.hunks.push(hunk);
        }
    }
    if let Some(f) = current_file.take() {
        files.push(f);
    }

    Diff { files }
}

fn parse_hunk_header(line: &str) -> (usize, usize) {
    let mut old_start = 0;
    let mut new_start = 0;
    for part in line.split_whitespace() {
        if let Some(rest) = part.strip_prefix('-') {
            old_start = rest
                .split(',')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if let Some(rest) = part.strip_prefix('+') {
            new_start = rest
                .split(',')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    (old_start, new_start)
}

pub fn fetch_commits(pr_number: u64) -> Vec<Commit> {
    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &pr_number.to_string(),
            "--json",
            "commits",
        ])
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

fn map_pr(gh: GhPr) -> PullRequest {
    PullRequest {
        id: gh.number,
        title: gh.title,
        description: gh.body,
        author: User {
            id: gh.author.login.clone(),
            username: gh.author.login,
            display_name: gh.author.name,
            avatar_url: None,
        },
        repo: Repo {
            id: String::new(),
            name: String::new(),
            full_name: String::new(),
            remote_url: String::new(),
            provider: ProviderKind::GitHub,
        },
        ci: CiStatus {
            state: CiState::Unknown,
            description: None,
            url: None,
        },
        status: if gh.is_draft {
            PrStatus::Draft
        } else {
            match gh.state.as_str() {
                "OPEN" => PrStatus::Open,
                "MERGED" => PrStatus::Merged,
                "CLOSED" => PrStatus::Declined,
                _ => PrStatus::Open,
            }
        },
        reviewers: vec![],
        build_status: None,
        comment_count: 0,
        source_branch: gh.head_ref_name,
        target_branch: gh.base_ref_name,
        files_changed: vec![],
        created: gh.created_at,
        updated: gh.updated_at,
    }
}
