use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::domain::{
    ci::CiSummary,
    pr::{PrBatch, PrStatus, PullRequest},
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

/// How many merged and how many declined PRs one read returns, newest first.
/// Open PRs are always read in full; closed ones are history, so they come a
/// page of each kind at a time.
const RECENT_CLOSED_PER_STATE: u32 = 25;

fn list_path(config: &Config) -> String {
    format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests",
        config.repo.project_key, config.repo.repo_slug
    )
}

/// Every open PR and the first page of merged and of declined ones.
pub fn fetch_prs(config: &Config) -> Result<PrBatch, FetchError> {
    let path = format!("{}?state=OPEN&limit=50", list_path(config));
    let mut prs: Vec<BbPr> = get_all(&config.repo.base_url, &path, &config.pat)?;
    let (closed, more) = closed_pages(config, [Some(0), Some(0)])?;
    prs.extend(closed);
    Ok(PrBatch {
        prs: prs.into_iter().map(map_pr).collect(),
        more,
    })
}

/// The pages of merged and declined PRs after the positions an earlier read
/// returned.
pub fn fetch_older_prs(config: &Config, after: &str) -> Result<PrBatch, FetchError> {
    let (closed, more) = closed_pages(config, decode_position(after)?)?;
    Ok(PrBatch {
        prs: closed.into_iter().map(map_pr).collect(),
        more,
    })
}

const CLOSED_STATES: [&str; 2] = ["MERGED", "DECLINED"];

/// Read the next page of each closed kind that still has one. The two kinds
/// are separate streams, so the position to continue from holds one offset for
/// each.
fn closed_pages(
    config: &Config,
    from: [Option<u64>; 2],
) -> Result<(Vec<BbPr>, Option<String>), FetchError> {
    let mut prs = Vec::new();
    let mut next = [None, None];
    for (i, state) in CLOSED_STATES.iter().enumerate() {
        let Some(start) = from[i] else { continue };
        let path = format!(
            "{}?state={state}&limit={RECENT_CLOSED_PER_STATE}",
            list_path(config)
        );
        let (values, more) =
            get_page_from::<BbPr>(&config.repo.base_url, &path, &config.pat, start)?;
        prs.extend(values);
        next[i] = more;
    }
    Ok((prs, encode_position(next)))
}

/// `"25|"`: continue merged at offset 25, declined is done. `None` when both are.
fn encode_position(next: [Option<u64>; 2]) -> Option<String> {
    let part = |n: Option<u64>| n.map(|n| n.to_string()).unwrap_or_default();
    next.iter()
        .any(Option::is_some)
        .then(|| format!("{}|{}", part(next[0]), part(next[1])))
}

fn decode_position(text: &str) -> Result<[Option<u64>; 2], FetchError> {
    let unreadable = || FetchError::InvalidInput("Unreadable position for older PRs.".into());
    let (merged, declined) = text.split_once('|').ok_or_else(unreadable)?;
    let part = |part: &str| {
        if part.is_empty() {
            Ok(None)
        } else {
            part.parse().map(Some).map_err(|_| unreadable())
        }
    };
    Ok([part(merged)?, part(declined)?])
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
        url: bb.links.web.into_iter().next().map(|link| link.href),
        id: bb.id,
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
