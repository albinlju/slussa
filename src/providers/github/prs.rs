use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::ci::CiSummary;
use crate::domain::pr::{PrStatus, PullRequest};
use crate::domain::review::{Reviewer, ReviewerState};
use crate::domain::user::User;
use crate::providers::error::FetchError;

#[derive(Debug, Default, Deserialize)]
struct GhAuthor {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Count {
    total_count: u32,
}
#[derive(Debug, Deserialize)]
struct Connection<T> {
    nodes: Vec<T>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}
#[derive(Debug, Deserialize)]
struct PageInfo {
    #[serde(rename = "hasNextPage")]
    has_next_page: bool,
}
#[derive(Debug, Default, Deserialize)]
struct Commits {
    nodes: Vec<CommitNode>,
}
#[derive(Debug, Deserialize)]
struct CommitNode {
    commit: CommitStatus,
}
#[derive(Debug, Deserialize)]
struct CommitStatus {
    #[serde(rename = "statusCheckRollup")]
    status_check_rollup: Option<Rollup>,
}
#[derive(Debug, Deserialize)]
struct Rollup {
    state: String,
}

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

/// Outstanding review requests. A request for a team has no `login` and is
/// skipped; only people are listed as reviewers.
#[derive(Debug, Default, Deserialize)]
struct ReviewRequests {
    #[serde(default)]
    nodes: Vec<ReviewRequest>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRequest {
    requested_reviewer: Option<RequestedReviewer>,
}
#[derive(Debug, Default, Deserialize)]
struct RequestedReviewer {
    login: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCheck {
    #[serde(default)]
    conclusion: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPr {
    url: Option<String>,
    id: String,
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
    comments: Count,
    latest_reviews: Connection<GhReviewSummary>,
    #[serde(default)]
    review_requests: ReviewRequests,
    #[serde(default)]
    commits: Commits,
    labels: Connection<GhLabel>,
}

/// What the list needs of each pull request. Connections nested here are
/// capped at 100; `fetch_prs` refetches the ones that report more.
const PR_FIELDS: &str = r"
    id url title number
    author { login }
    state isDraft headRefName baseRefName body createdAt updatedAt
    additions deletions changedFiles
    comments { totalCount }
    latestReviews(first: 100) { nodes { state author { login } } pageInfo { hasNextPage } }
    labels(first: 100) { nodes { name } pageInfo { hasNextPage } }
    reviewRequests(first: 100) { nodes { requestedReviewer { ... on User { login } } } }
    commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
";

pub fn fetch_prs() -> Result<Vec<PullRequest>, FetchError> {
    let mut prs: Vec<GhPr> = super::pagination::repo_nodes("pullRequests", PR_FIELDS)?;
    for pr in &mut prs {
        if pr.labels.page_info.has_next_page {
            pr.labels.nodes =
                super::pagination::node_nodes(&pr.id, "PullRequest", "labels", "name")?;
        }
        if pr.latest_reviews.page_info.has_next_page {
            pr.latest_reviews.nodes = super::pagination::node_nodes(
                &pr.id,
                "PullRequest",
                "latestReviews",
                "state author { login }",
            )?;
        }
    }
    // Match the list's existing newest-first presentation.
    prs.sort_by_key(|pr| std::cmp::Reverse(pr.created_at));
    Ok(prs.into_iter().map(map_pr).collect())
}

fn map_pr(gh: GhPr) -> PullRequest {
    let checks: Vec<GhCheck> = gh
        .commits
        .nodes
        .into_iter()
        .filter_map(|n| n.commit.status_check_rollup)
        .map(|s| GhCheck {
            state: s.state,
            conclusion: String::new(),
            status: String::new(),
        })
        .collect();
    let ci_state = summarize_checks(&checks);
    let reviewers = with_requests(map_reviewers(gh.latest_reviews.nodes), gh.review_requests);
    let comment_count = gh.comments.total_count;

    PullRequest {
        url: gh.url,
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
                "MERGED" => PrStatus::Merged,
                "CLOSED" => PrStatus::Declined,
                _ => PrStatus::Open,
            }
        },
        reviewers,
        labels: gh.labels.nodes.into_iter().map(|l| l.name).collect(),
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

fn summarize_checks(checks: &[GhCheck]) -> CiSummary {
    if checks.is_empty() {
        return CiSummary::Unknown;
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
                any_failure = true;
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
        CiSummary::Failed
    } else if any_pending {
        CiSummary::Pending
    } else if any_success {
        CiSummary::Success
    } else {
        CiSummary::Unknown
    }
}

/// An outstanding request supersedes that person's earlier review: GitHub shows
/// a re-requested reviewer as pending again.
fn with_requests(mut reviewers: Vec<Reviewer>, requests: ReviewRequests) -> Vec<Reviewer> {
    for login in requests
        .nodes
        .into_iter()
        .filter_map(|request| request.requested_reviewer?.login)
    {
        reviewers.retain(|r| !r.author.username.eq_ignore_ascii_case(&login));
        reviewers.push(Reviewer {
            author: User { username: login },
            state: ReviewerState::Requested,
        });
    }
    reviewers
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

#[cfg(test)]
mod link_tests {
    use super::*;
    #[test]
    fn preserves_enterprise_pr_url_without_assuming_github_com() {
        let json = serde_json::json!({
            "id": "PR_42", "number": 42, "title": "PR", "state": "OPEN", "author": {"login": "alice"},
            "url": "https://github.example.com/team/repo/pull/42",
            "headRefName": "feature", "baseRefName": "main",
            "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
            "comments": {"totalCount": 0},
            "latestReviews": {"nodes": [], "pageInfo": {"hasNextPage": false}},
            "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}}
        });
        assert_eq!(
            map_pr(serde_json::from_value(json).unwrap()).url.as_deref(),
            Some("https://github.example.com/team/repo/pull/42")
        );
    }
}
