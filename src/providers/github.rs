use crate::domain::ci::{CiState, CiStatus};
use crate::domain::commit::Commit;
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
