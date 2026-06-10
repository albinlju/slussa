use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh_json;
use crate::domain::ci::CiState;
use crate::domain::pr::{PrStatus, PullRequest};
use crate::domain::review::{Reviewer, ReviewerState};
use crate::domain::user::User;

#[derive(Debug, Default, Deserialize)]
struct GhAuthor {
    #[serde(default)]
    login: String,
}

/// Deliberately empty — only the array length is used.
#[derive(Debug, Deserialize)]
struct GhCommentSummary {}

#[derive(Debug, Deserialize)]
struct GhLabel {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewSummary {
    #[serde(default)]
    state: String,
    #[serde(default)]
    author: GhAuthor,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCheck {
    /// Set for GitHub Actions check runs.
    #[serde(default)]
    conclusion: String,
    /// Set for GitHub Actions check runs.
    #[serde(default)]
    status: String,
    /// Set for external status checks.
    #[serde(default)]
    state: String,
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
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
    #[serde(default)]
    changed_files: u32,
    #[serde(default)]
    comments: Vec<GhCommentSummary>,
    #[serde(default)]
    latest_reviews: Vec<GhReviewSummary>,
    #[serde(default)]
    status_check_rollup: Vec<GhCheck>,
    #[serde(default)]
    labels: Vec<GhLabel>,
}

pub fn fetch_prs() -> Result<Vec<PullRequest>, FetchError> {
    // `gh pr list` defaults to open PRs and 30 results; match the Bitbucket
    // backend (all states, 50).
    let gh_prs: Vec<GhPr> = run_gh_json(&[
        "pr",
        "list",
        "--state",
        "all",
        "--limit",
        "50",
        "--json",
        "title,number,author,state,isDraft,headRefName,baseRefName,body,\
         createdAt,updatedAt,additions,deletions,changedFiles,comments,\
         latestReviews,statusCheckRollup,labels",
    ])?;
    Ok(gh_prs.into_iter().map(map_pr).collect())
}

fn map_pr(gh: GhPr) -> PullRequest {
    let ci_state = summarize_checks(&gh.status_check_rollup);
    let reviewers = map_reviewers(gh.latest_reviews);
    let comment_count = gh.comments.len() as u32;

    PullRequest {
        id: gh.number,
        title: gh.title,
        description: gh.body,
        author: User {
            username: gh.author.login,
        },
        ci: ci_state,
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
        reviewers,
        labels: gh.labels.into_iter().map(|l| l.name).collect(),
        comment_count,
        source_branch: gh.head_ref_name,
        target_branch: gh.base_ref_name,
        additions: gh.additions,
        deletions: gh.deletions,
        changed_files: gh.changed_files,
        created: gh.created_at,
        updated: gh.updated_at,
    }
}

fn summarize_checks(checks: &[GhCheck]) -> CiState {
    if checks.is_empty() {
        return CiState::Unknown;
    }

    let mut any_failure = false;
    let mut any_pending = false;
    let mut any_success = false;

    for c in checks {
        let outcome = if !c.conclusion.is_empty() {
            c.conclusion.as_str()
        } else if !c.state.is_empty() {
            c.state.as_str()
        } else {
            ""
        };

        match outcome {
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" => {
                any_failure = true
            }
            "SUCCESS" => any_success = true,
            "PENDING" | "QUEUED" | "IN_PROGRESS" => any_pending = true,
            _ => {}
        }

        if c.status == "IN_PROGRESS" || c.status == "QUEUED" {
            any_pending = true;
        }
    }

    if any_failure {
        CiState::Failed
    } else if any_pending {
        CiState::Pending
    } else if any_success {
        CiState::Success
    } else {
        CiState::Unknown
    }
}

fn map_reviewers(reviews: Vec<GhReviewSummary>) -> Vec<Reviewer> {
    reviews
        .into_iter()
        .map(|r| {
            let state = match r.state.as_str() {
                "APPROVED" => ReviewerState::Approved,
                "CHANGES_REQUESTED" => ReviewerState::ChangesRequested,
                _ => ReviewerState::Commented,
            };
            Reviewer {
                author: User {
                    username: r.author.login,
                },
                state,
            }
        })
        .collect()
}
