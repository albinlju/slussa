use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::domain::{
    ci::CiSummary,
    pr::{PrBatch, PrGroup, PrId, PrStatus, PullRequest},
    review::{Reviewer, ReviewerState},
    user::User,
};
use crate::providers::bitbucket_dc::http::{get_all, get_page_from};
use crate::providers::error::FetchError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbPr {
    #[serde(default)]
    links: BbLinks,
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
struct BbLinks {
    #[serde(rename = "self", default)]
    web: Vec<BbLink>,
}
#[derive(Debug, Deserialize)]
struct BbLink {
    href: String,
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

/// How many merged or declined PRs one page holds, newest first.
const CLOSED_PAGE: u32 = 25;

/// One group of the repository's PRs. The open group, drafts included, is read
/// in full and `after` is ignored. A closed group is one page from the offset in
/// `after` (from the start when it is `None`), with the next offset in `more`.
pub fn fetch_prs(
    config: &Config,
    group: PrGroup,
    after: Option<&str>,
) -> Result<PrBatch, FetchError> {
    let list = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests",
        config.repo.project_key, config.repo.repo_slug
    );
    let (base_url, pat) = (&config.repo.base_url, &config.pat);
    let (values, more): (Vec<BbPr>, Option<String>) = match group {
        PrGroup::Open => (
            get_all(base_url, &format!("{list}?state=OPEN&limit=50"), pat)?,
            None,
        ),
        PrGroup::Merged | PrGroup::Declined => {
            let state = if group == PrGroup::Merged {
                "MERGED"
            } else {
                "DECLINED"
            };
            let start = after.map_or(Ok(0), |after| {
                after.parse::<u64>().map_err(|e| {
                    tracing::warn!("unreadable position {after:?} for older PRs: {e}");
                    FetchError::InvalidInput("Unreadable position for older PRs.".into())
                })
            })?;
            let path = format!("{list}?state={state}&limit={CLOSED_PAGE}");
            let (values, next) = get_page_from::<BbPr>(base_url, &path, pat, start)?;
            (values, next.map(|next| next.to_string()))
        }
    };
    Ok(PrBatch {
        prs: values.into_iter().map(map_pr).collect(),
        more,
    })
}

fn map_pr(bb: BbPr) -> PullRequest {
    // Only an open PR is a draft, whatever the flag says on a closed one.
    let status = match bb.state.as_str() {
        "MERGED" => PrStatus::Merged,
        "DECLINED" => PrStatus::Declined,
        _ if bb.draft => PrStatus::Draft,
        _ => PrStatus::Open,
    };

    PullRequest {
        url: bb.links.web.into_iter().next().map(|link| link.href),
        id: PrId(bb.id),
        title: bb.title,
        description: bb.description,
        author: map_user(bb.author.user),
        ci: CiSummary::Unknown,
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
        // UNAPPROVED: a listed reviewer who has not reviewed.
        _ => ReviewerState::Requested,
    };
    Reviewer {
        author: map_user(r.user),
        state,
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;
    #[test]
    fn uses_server_web_link_including_context_path_and_handles_missing_links() {
        let mut json = serde_json::json!({
            "id": 42, "title": "PR", "state": "OPEN", "createdDate": 0, "updatedDate": 0,
            "fromRef": {"displayId": "feature"}, "toRef": {"displayId": "main"},
            "author": {"user": {"name": "alice"}},
            "links": {"self": [{"href": "https://code.example.com/context/projects/TEAM/repos/repo/pull-requests/42/overview"}]}
        });
        let pr = map_pr(serde_json::from_value(json.clone()).unwrap());
        assert_eq!(
            pr.url.as_deref(),
            Some(
                "https://code.example.com/context/projects/TEAM/repos/repo/pull-requests/42/overview"
            )
        );
        json.as_object_mut().unwrap().remove("links");
        assert!(map_pr(serde_json::from_value(json).unwrap()).url.is_none());
    }
}
