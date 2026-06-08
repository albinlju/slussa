use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::{
    ci::{CiState, CiStatus},
    pr::{PrStatus, PullRequest},
    provider::ProviderKind,
    repo::Repo,
    review::{Reviewer, ReviewerState},
    user::User,
};

#[derive(Debug, Deserialize)]
struct PagedPrs {
    values: Vec<BbPr>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbPr {
    id: u64,
    title: String,
    #[serde(default)]
    description: Option<String>,
    state: String,
    #[serde(default)]
    draft: bool,
    created_date: i64,
    updated_date: i64,
    from_ref: BbRef,
    to_ref: BbRef,
    author: BbAuthor,
    #[serde(default)]
    reviewers: Vec<BbReviewer>,
    #[serde(default)]
    properties: BbProps,
}

#[derive(Debug, Default, Deserialize)]
struct BbProps {
    #[serde(rename = "commentCount", default)]
    comment_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbRef {
    display_id: String,
}

#[derive(Debug, Deserialize)]
struct BbAuthor {
    user: BbUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbUser {
    /// Username — what `@-mentions` use.
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BbReviewer {
    user: BbUser,
    #[serde(default)]
    status: String,
}

pub fn fetch_prs(config: &Config) -> Result<Vec<PullRequest>, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests?state=ALL&limit=50",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: PagedPrs = get_json(&config.repo.host, &path, &config.pat)?;
    Ok(page.values.into_iter().map(map_pr).collect())
}

fn map_pr(bb: BbPr) -> PullRequest {
    let status = if bb.draft {
        PrStatus::Draft
    } else {
        match bb.state.as_str() {
            "OPEN" => PrStatus::Open,
            "MERGED" => PrStatus::Merged,
            "DECLINED" => PrStatus::Declined,
            _ => PrStatus::Open,
        }
    };

    PullRequest {
        id: bb.id,
        title: bb.title,
        description: bb.description,
        author: map_user(bb.author.user),
        repo: Repo {
            id: String::new(),
            name: String::new(),
            full_name: String::new(),
            remote_url: String::new(),
            provider: ProviderKind::Bitbucket,
        },
        // Bitbucket DC doesn't surface CI on the PR list endpoint — needs a
        // separate /builds call. Left Unknown for now.
        ci: CiStatus {
            state: CiState::Unknown,
            description: None,
            url: None,
        },
        status,
        reviewers: bb.reviewers.into_iter().map(map_reviewer).collect(),
        // Bitbucket DC has no first-class label concept on PRs.
        labels: Vec::new(),
        build_status: None,
        comment_count: bb.properties.comment_count,
        source_branch: bb.from_ref.display_id,
        target_branch: bb.to_ref.display_id,
        files_changed: vec![],
        additions: 0,
        deletions: 0,
        changed_files: 0,
        created: ms_to_utc(bb.created_date),
        updated: ms_to_utc(bb.updated_date),
    }
}

fn map_user(u: BbUser) -> User {
    User {
        id: u.name.clone(),
        username: u.name,
        display_name: u.display_name,
        avatar_url: None,
    }
}

fn map_reviewer(r: BbReviewer) -> Reviewer {
    let state = match r.status.as_str() {
        "APPROVED" => ReviewerState::Approved,
        "NEEDS_WORK" => ReviewerState::ChangesRequested,
        _ => ReviewerState::Commented,
    };
    let username = r.user.name.clone();
    Reviewer {
        id: username.clone(),
        author: map_user(r.user),
        state,
        body: None,
    }
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
