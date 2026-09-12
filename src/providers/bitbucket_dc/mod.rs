mod activities;
pub mod auth;
mod builds;
mod comments;
mod commits;
mod diff;
mod http;
mod json_diff;
mod probe;
mod prs;
pub mod remote;

use chrono::{DateTime, TimeZone, Utc};

use crate::domain::pr::Mergeability;
use crate::domain::review::{ReviewComment, ReviewVerdict};
use crate::providers::error::FetchError;

pub use activities::fetch as fetch_activity;
pub use builds::fetch_builds;
pub use comments::{
    delete_comment, edit_comment, post_comment, post_pr_comment, reply_comment, set_thread_resolved,
};
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use probe::is_instance;
pub use prs::fetch_prs;

pub(super) const APP_PROPERTIES_PATH: &str = "/rest/api/1.0/application-properties";

pub fn current_user(config: &Config) -> Result<String, FetchError> {
    http::current_user(&config.repo.base_url, APP_PROPERTIES_PATH, &config.pat)
}

pub fn submit_review(
    config: &Config,
    pr_id: u64,
    verdict: ReviewVerdict,
    body: &str,
    user: &str,
) -> Result<(), FetchError> {
    // Bitbucket has no review body — post any summary as a PR comment first.
    if !body.is_empty() {
        comments::post_pr_comment(config, pr_id, body)?;
    }
    let status = match verdict {
        ReviewVerdict::Approve => "APPROVED",
        ReviewVerdict::RequestChanges => "NEEDS_WORK",
        ReviewVerdict::Unapprove => "UNAPPROVED",
        // A plain comment review is just the comment posted above — no status flip.
        ReviewVerdict::Comment => return Ok(()),
    };
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/participants/{user}",
        config.repo.project_key, config.repo.repo_slug,
    );
    let payload = serde_json::json!({ "status": status });
    http::put_json(&config.repo.base_url, &endpoint, &config.pat, &payload).map_err(|source| {
        FetchError::PartialReview {
            posted_comments: 0,
            summary_posted: !body.is_empty(),
            source: Box::new(source),
        }
    })
}

/// Bitbucket has no batched review: post each queued line comment, then submit
/// the summary + status. This is *not* atomic — if a later step fails the
/// earlier posts remain, and the surfaced error is whichever step failed.
pub fn submit_full_review(
    config: &Config,
    pr_id: u64,
    verdict: ReviewVerdict,
    body: &str,
    user: &str,
    comments: &[ReviewComment],
) -> Result<(), FetchError> {
    if comments.iter().any(|c| c.revision.is_none()) {
        return Err(FetchError::InvalidInput(
            "Comment revision is unknown; reload the diff.".into(),
        ));
    }
    publish_steps(
        comments,
        |c| {
            comments::post_comment(
                config,
                pr_id,
                &c.path,
                c.line,
                c.removed,
                &c.body,
                c.revision.as_ref().expect("validated revision"),
            )
        },
        || submit_review(config, pr_id, verdict, body, user),
    )
}

fn publish_steps<T>(
    comments: &[T],
    mut post: impl FnMut(&T) -> Result<(), FetchError>,
    finish: impl FnOnce() -> Result<(), FetchError>,
) -> Result<(), FetchError> {
    for (i, comment) in comments.iter().enumerate() {
        if let Err(source) = post(comment) {
            return Err(FetchError::PartialReview {
                posted_comments: i,
                summary_posted: false,
                source: Box::new(source),
            });
        }
    }
    finish().map_err(|error| match error {
        FetchError::PartialReview {
            summary_posted,
            source,
            ..
        } => FetchError::PartialReview {
            posted_comments: comments.len(),
            summary_posted,
            source,
        },
        source => FetchError::PartialReview {
            posted_comments: comments.len(),
            summary_posted: false,
            source: Box::new(source),
        },
    })
}

pub fn fetch_mergeability(config: &Config, pr_id: u64) -> Result<Mergeability, FetchError> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MergeStatus {
        can_merge: bool,
        conflicted: bool,
    }
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/merge",
        config.repo.project_key, config.repo.repo_slug,
    );
    let status: MergeStatus = http::get_json(&config.repo.base_url, &endpoint, &config.pat)?;
    // A veto without a conflict (e.g. missing approvals) is "can't merge yet" but
    // not a conflict — coarse `Unknown` covers it until we model it explicitly.
    Ok(if status.conflicted {
        Mergeability::Conflicts
    } else if status.can_merge {
        Mergeability::Mergeable
    } else {
        Mergeability::Unknown
    })
}

/// Bitbucket's merge/decline endpoints take the PR's current version for
/// optimistic locking, so read it fresh before either.
fn pr_version(config: &Config, pr_id: u64) -> Result<u64, FetchError> {
    #[derive(serde::Deserialize)]
    struct PrVersion {
        version: u64,
    }
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}",
        config.repo.project_key, config.repo.repo_slug,
    );
    let pr: PrVersion = http::get_json(&config.repo.base_url, &endpoint, &config.pat)?;
    Ok(pr.version)
}

pub fn merge(config: &Config, pr_id: u64) -> Result<(), FetchError> {
    // Strategy is the repo's configured default.
    let version = pr_version(config, pr_id)?;
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/merge?version={version}",
        config.repo.project_key, config.repo.repo_slug,
    );
    http::post_json(
        &config.repo.base_url,
        &endpoint,
        &config.pat,
        &serde_json::json!({}),
    )
}

pub fn decline(config: &Config, pr_id: u64) -> Result<(), FetchError> {
    let version = pr_version(config, pr_id)?;
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/decline?version={version}",
        config.repo.project_key, config.repo.repo_slug,
    );
    http::post_json(
        &config.repo.base_url,
        &endpoint,
        &config.pat,
        &serde_json::json!({}),
    )
}

#[derive(Clone, Debug)]
pub struct RepoLocation {
    pub base_url: String,
    pub project_key: String,
    pub repo_slug: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub repo: RepoLocation,
    pub pat: String,
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}

#[cfg(test)]
mod submission_tests {
    use super::*;
    #[test]
    fn partial_failure_reports_only_acknowledged_steps() {
        let result = publish_steps(
            &[1, 2, 3],
            |n| {
                if *n == 3 {
                    Err(FetchError::Network("offline".into()))
                } else {
                    Ok(())
                }
            },
            || panic!("must not submit verdict"),
        );
        assert!(matches!(
            result,
            Err(FetchError::PartialReview {
                posted_comments: 2,
                summary_posted: false,
                ..
            })
        ));
        let result = publish_steps(
            &[1, 2],
            |_| Ok(()),
            || {
                Err(FetchError::PartialReview {
                    posted_comments: 0,
                    summary_posted: true,
                    source: Box::new(FetchError::Network("offline".into())),
                })
            },
        );
        assert!(matches!(
            result,
            Err(FetchError::PartialReview {
                posted_comments: 2,
                summary_posted: true,
                ..
            })
        ));
    }
}
