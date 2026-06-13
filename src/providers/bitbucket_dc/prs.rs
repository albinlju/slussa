use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::providers::bitbucket_dc::http::get_json;
use crate::providers::error::FetchError;
use crate::domain::{
    ci::CiState,
    pr::{PrStatus, PullRequest},
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
struct BbUser {
    name: String,
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
    let page: PagedPrs = get_json(&config.repo.base_url, &path, &config.pat)?;
    Ok(page.values.into_iter().map(map_pr).collect())
}

fn map_pr(bb: BbPr) -> PullRequest {
    let status = if bb.draft {
        PrStatus::Draft
    } else {
        match bb.state.as_str() {
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
        ci: CiState::Unknown,
        status,
        reviewers: bb.reviewers.into_iter().map(map_reviewer).collect(),
        labels: Vec::new(),
        comment_count: bb.properties.comment_count,
        source_branch: bb.from_ref.display_id,
        target_branch: bb.to_ref.display_id,
        additions: 0,
        deletions: 0,
        changed_files: 0,
        created: ms_to_utc(bb.created_date),
        updated: ms_to_utc(bb.updated_date),
    }
}

fn map_user(u: BbUser) -> User {
    User { username: u.name }
}

fn map_reviewer(r: BbReviewer) -> Reviewer {
    let state = match r.status.as_str() {
        "APPROVED" => ReviewerState::Approved,
        "NEEDS_WORK" => ReviewerState::ChangesRequested,
        _ => ReviewerState::Commented,
    };
    Reviewer {
        author: map_user(r.user),
        state,
    }
}
