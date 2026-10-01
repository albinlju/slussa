use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::ci::CiSummary;
use crate::domain::pr::{PrBatch, PrGroup, PrId, PrInfo, PrStatus, PullRequest};
use crate::domain::review::{Reviewer, ReviewerState};
use crate::domain::user::User;
use crate::providers::error::FetchError;

use super::pagination::Connection;

#[derive(Debug, Default, Deserialize)]
struct GhAuthor {
    #[serde(default)]
    login: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Count {
    total_count: u32,
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
struct GhPr {
    url: Option<String>,
    id: String,
    number: u64,
    title: String,
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
    /// Threads on lines of code. Each one counts once however long it is, so the
    /// list shows how many conversations a PR has, not how many posts.
    #[serde(default)]
    review_threads: Count,
    latest_reviews: Connection<GhReviewSummary>,
    #[serde(default)]
    review_requests: ReviewRequests,
    #[serde(default)]
    commits: Commits,
}

/// What the list needs of each pull request. The description and the labels
/// are left out: the list shows neither, and they made a page about twice as
/// slow (ROADMAP.md); `fetch_info` reads them when a PR is opened.
/// Connections nested here are capped at 100; `fetch_prs` refetches the ones
/// that report more.
const PR_FIELDS: &str = r"
    id url title number
    author { login }
    state isDraft headRefName baseRefName createdAt updatedAt
    additions deletions changedFiles
    comments { totalCount }
    reviewThreads { totalCount }
    latestReviews(first: 100) { nodes { state author { login } } pageInfo { hasNextPage } }
    reviewRequests(first: 100) { nodes { requestedReviewer { ... on User { login } } } }
    commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
";

/// How many PRs one request asks for, and how many a closed page holds, most
/// recently updated first. GitHub gives a GraphQL request about ten seconds,
/// and with this selection 100 PRs took 7 to 11 seconds on a large repository
/// and once timed out, while 30 took about 2 (measured on cli/cli, 2026-09-30).
const PAGE: u32 = 30;
const OPEN_ARGS: &str = "states: OPEN, ";
const MERGED_ARGS: &str = "states: MERGED, orderBy: {field: UPDATED_AT, direction: DESC}, ";
const DECLINED_ARGS: &str = "states: CLOSED, orderBy: {field: UPDATED_AT, direction: DESC}, ";

/// One page of a group of the repository's PRs, after `after`, and the
/// position to continue from in `more` (`None` at the end). The open group is
/// read the same way, a page at a time, so the caller can show the first page
/// while the rest are read.
pub fn fetch_prs(group: PrGroup, after: Option<&str>) -> Result<PrBatch, FetchError> {
    let args = match group {
        PrGroup::Open => OPEN_ARGS,
        PrGroup::Merged => MERGED_ARGS,
        PrGroup::Declined => DECLINED_ARGS,
    };
    let (nodes, more) = one_page(args, after)?;
    Ok(PrBatch {
        prs: complete(nodes)?,
        more,
    })
}

fn one_page(args: &str, after: Option<&str>) -> Result<(Vec<GhPr>, Option<String>), FetchError> {
    super::pagination::repo_page("pullRequests", args, PR_FIELDS, PAGE, after)
}

/// Read the nested connections the list query capped at 100, order newest
/// first and map to the domain.
fn complete(mut prs: Vec<GhPr>) -> Result<Vec<PullRequest>, FetchError> {
    for pr in &mut prs {
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

/// The description and labels of one PR.
pub fn fetch_info(pr: PrId) -> Result<PrInfo, FetchError> {
    #[derive(Deserialize)]
    struct Fields {
        id: String,
        #[serde(default)]
        body: Option<String>,
        labels: Connection<GhLabel>,
    }
    let fields: Fields = super::run_pr_graphql(super::graphql::INFO, pr)?;
    let labels = if fields.labels.page_info.has_next_page {
        super::pagination::node_nodes(&fields.id, "PullRequest", "labels", "name")?
    } else {
        fields.labels.nodes
    };
    Ok(PrInfo {
        description: fields.body,
        labels: labels.into_iter().map(|label| label.name).collect(),
    })
}

fn map_pr(gh: GhPr) -> PullRequest {
    let ci_state = summarize_checks(
        gh.commits
            .nodes
            .into_iter()
            .filter_map(|node| node.commit.status_check_rollup)
            .map(|rollup| rollup.state),
    );
    let reviewers = with_requests(map_reviewers(gh.latest_reviews.nodes), gh.review_requests);
    let comment_count = gh.comments.total_count + gh.review_threads.total_count;

    PullRequest {
        url: gh.url,
        id: PrId(gh.number),
        title: gh.title,
        description: None,
        author: User {
            username: gh.author.login,
        },
        ci: ci_state,
        // Only an open PR is a draft: GitHub keeps the flag on a closed one.
        status: match gh.state.as_str() {
            "MERGED" => PrStatus::Merged,
            "CLOSED" => PrStatus::Declined,
            _ if gh.is_draft => PrStatus::Draft,
            _ => PrStatus::Open,
        },
        reviewers,
        labels: Vec::new(),
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

/// A PR's CI state from the status rollups of the commits read for it (the
/// last one). No rollup, or one in a state not known here, is `Unknown`.
fn summarize_checks(states: impl IntoIterator<Item = String>) -> CiSummary {
    let mut any_failure = false;
    let mut any_pending = false;
    let mut any_success = false;

    for state in states {
        match state.as_str() {
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" => {
                any_failure = true;
            }
            "SUCCESS" => any_success = true,
            "PENDING" | "QUEUED" | "IN_PROGRESS" => any_pending = true,
            _ => {}
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
            "latestReviews": {"nodes": [], "pageInfo": {"hasNextPage": false}}
        });
        assert_eq!(
            map_pr(serde_json::from_value(json).unwrap()).url.as_deref(),
            Some("https://github.example.com/team/repo/pull/42")
        );
    }
}

#[cfg(test)]
mod comment_count_tests {
    use super::*;

    /// A PR with one conversation comment and, if given, that many threads on code.
    fn comment_count(threads: Option<u32>) -> u32 {
        let mut json = serde_json::json!({
            "id": "PR_1", "number": 1, "title": "PR", "state": "OPEN", "author": {"login": "a"},
            "url": "https://github.com/o/r/pull/1",
            "headRefName": "f", "baseRefName": "main",
            "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
            "comments": {"totalCount": 1},
            "latestReviews": {"nodes": [], "pageInfo": {"hasNextPage": false}}
        });
        if let (Some(n), Some(object)) = (threads, json.as_object_mut()) {
            object.insert("reviewThreads".into(), serde_json::json!({"totalCount": n}));
        }
        map_pr(serde_json::from_value(json).unwrap()).comment_count
    }

    #[test]
    fn threads_on_code_count_beside_the_conversation_comments() {
        assert_eq!(comment_count(Some(2)), 3);
    }

    #[test]
    fn without_threads_only_the_conversation_comments_count() {
        assert_eq!(comment_count(None), 1);
        assert_eq!(comment_count(Some(0)), 1);
    }
}

#[cfg(test)]
mod ci_tests {
    use super::*;

    fn summary(states: &[&str]) -> CiSummary {
        summarize_checks(states.iter().map(|state| (*state).to_owned()))
    }

    #[test]
    fn the_rollup_state_becomes_the_ci_summary_and_a_failure_outweighs_the_rest() {
        assert_eq!(summary(&[]), CiSummary::Unknown, "no checks at all");
        assert_eq!(summary(&["SUCCESS"]), CiSummary::Success);
        assert_eq!(summary(&["PENDING"]), CiSummary::Pending);
        assert_eq!(summary(&["FAILURE"]), CiSummary::Failed);
        assert_eq!(summary(&["ERROR"]), CiSummary::Failed);
        assert_eq!(summary(&["EXPECTED"]), CiSummary::Unknown);
        assert_eq!(summary(&["SUCCESS", "PENDING"]), CiSummary::Pending);
        assert_eq!(
            summary(&["SUCCESS", "PENDING", "FAILURE"]),
            CiSummary::Failed
        );
    }
}
