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

use crate::domain::pr::{Mergeability, PrId};
use crate::domain::review::{ReviewComment, ReviewVerdict, ReviewedHead};
use crate::domain::user::Username;
use crate::providers::error::{FetchError, ReviewError};

pub use activities::fetch as fetch_activity;
pub use builds::fetch_builds;
pub use comments::{
    delete_comment, edit_comment, post_comment, post_pr_comment, reply_comment, set_thread_resolved,
};
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use probe::is_instance;
pub use prs::{fetch_pr, fetch_prs};

pub(super) const APP_PROPERTIES_PATH: &str = "/rest/api/1.0/application-properties";

pub fn current_user(config: &Config) -> Result<Username, FetchError> {
    http::current_user(&config.repo.base_url, APP_PROPERTIES_PATH, &config.pat)
}

pub fn submit_review(
    config: &Config,
    pr_id: PrId,
    verdict: ReviewVerdict,
    body: &str,
    user: &str,
    head: &ReviewedHead,
) -> Result<(), ReviewError> {
    // Bitbucket has no review body — post any summary as a PR comment first.
    if !body.is_empty() {
        post_pr_comment(config, pr_id, body)?;
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
    // An approval says which commit it is of.
    let payload = if verdict == ReviewVerdict::Approve {
        serde_json::json!({ "status": status, "lastReviewedCommit": head.as_str() })
    } else {
        serde_json::json!({ "status": status })
    };
    http::put_json(&config.repo.base_url, &endpoint, &config.pat, &payload).map_err(|source| {
        // Without a summary the verdict is the only request: its failure is
        // the whole failure, not part of one.
        if body.is_empty() {
            ReviewError::Failed(source)
        } else {
            ReviewError::Partial {
                posted_comments: 0,
                summary_posted: true,
                source,
            }
        }
    })
}

/// Bitbucket has no batched review: post each queued line comment, then submit
/// the summary + status. This is *not* atomic — if a later step fails the
/// earlier posts remain, and the surfaced error is whichever step failed.
pub fn submit_full_review(
    config: &Config,
    pr_id: PrId,
    verdict: ReviewVerdict,
    body: &str,
    user: &str,
    comments: &[ReviewComment],
    head: &ReviewedHead,
) -> Result<(), ReviewError> {
    // Before anything is posted: an approval of a branch that has moved is
    // refused whole, not after some of its comments arrived.
    if verdict == ReviewVerdict::Approve && pr_now(config, pr_id)?.head != head.as_str() {
        return Err(ReviewError::Failed(moved_since_read()));
    }
    publish_steps(
        comments,
        |comment| post_comment(config, pr_id, comment),
        || submit_review(config, pr_id, verdict, body, user, head),
    )
}

fn publish_steps<T>(
    comments: &[T],
    mut post: impl FnMut(&T) -> Result<(), FetchError>,
    finish: impl FnOnce() -> Result<(), ReviewError>,
) -> Result<(), ReviewError> {
    for (i, comment) in comments.iter().enumerate() {
        if let Err(source) = post(comment) {
            return Err(ReviewError::Partial {
                posted_comments: i,
                summary_posted: false,
                source,
            });
        }
    }
    // Every comment arrived, whatever happened to the summary and the verdict.
    finish().map_err(|error| match error {
        // With no comments before it, nothing arrived in part.
        ReviewError::Failed(source) if comments.is_empty() => ReviewError::Failed(source),
        ReviewError::Partial {
            summary_posted,
            source,
            ..
        } => ReviewError::Partial {
            posted_comments: comments.len(),
            summary_posted,
            source,
        },
        ReviewError::Failed(source) => ReviewError::Partial {
            posted_comments: comments.len(),
            summary_posted: false,
            source,
        },
    })
}

pub fn fetch_mergeability(config: &Config, pr_id: PrId) -> Result<Mergeability, FetchError> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Status {
        can_merge: bool,
        conflicted: bool,
        #[serde(default)]
        vetoes: Vec<Veto>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Veto {
        #[serde(default)]
        summary_message: String,
    }
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/merge",
        config.repo.project_key, config.repo.repo_slug,
    );
    let status: Status = http::get_json(&config.repo.base_url, &endpoint, &config.pat)?;
    let reasons: Vec<String> = status
        .vetoes
        .into_iter()
        .map(|veto| veto.summary_message)
        .filter(|message| !message.is_empty())
        .collect();
    let or_default = |default: &str| {
        if reasons.is_empty() {
            vec![default.to_owned()]
        } else {
            reasons.clone()
        }
    };
    // A veto without a conflict is a merge check: approvals, builds, tasks.
    Ok(if status.conflicted {
        Mergeability::Conflicts(or_default("It has merge conflicts."))
    } else if status.can_merge {
        Mergeability::Mergeable
    } else {
        Mergeability::Blocked(or_default("Merge checks have not passed."))
    })
}

/// Bitbucket's merge/decline endpoints take the PR's current version for
/// optimistic locking, so read it fresh before either.
fn pr_version(config: &Config, pr_id: PrId) -> Result<u64, FetchError> {
    Ok(pr_now(config, pr_id)?.version)
}

/// The PR as it is now: its version, and the commit its source branch is at.
struct PrNow {
    version: u64,
    head: String,
}

fn pr_now(config: &Config, pr_id: PrId) -> Result<PrNow, FetchError> {
    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Ref {
        #[serde(default)]
        latest_commit: String,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Pr {
        version: u64,
        #[serde(default)]
        from_ref: Ref,
    }
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}",
        config.repo.project_key, config.repo.repo_slug,
    );
    let pr: Pr = http::get_json(&config.repo.base_url, &endpoint, &config.pat)?;
    Ok(PrNow {
        version: pr.version,
        head: pr.from_ref.latest_commit,
    })
}

/// The answer to a verdict or a merge of a PR whose branch has moved since it
/// was read.
fn moved_since_read() -> FetchError {
    FetchError::Stale(
        "The PR changed after you read it. Read what is new in the Diff tab (F refreshes it), then try again.".into(),
    )
}

pub fn merge(config: &Config, pr_id: PrId, head: &ReviewedHead) -> Result<(), FetchError> {
    // Strategy is the repo's configured default. The version and the head come
    // from the same read, so the merge is of the head that was checked.
    let now = pr_now(config, pr_id)?;
    if now.head != head.as_str() {
        return Err(moved_since_read());
    }
    let version = now.version;
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

pub fn reopen(config: &Config, pr_id: PrId) -> Result<(), FetchError> {
    let version = pr_version(config, pr_id)?;
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/reopen?version={version}",
        config.repo.project_key, config.repo.repo_slug,
    );
    http::post_json(
        &config.repo.base_url,
        &endpoint,
        &config.pat,
        &serde_json::json!({}),
    )
}

pub fn decline(config: &Config, pr_id: PrId) -> Result<(), FetchError> {
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
    pub pat: auth::Pat,
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
            Err(ReviewError::Partial {
                posted_comments: 2,
                summary_posted: false,
                ..
            })
        ));
        let result = publish_steps(
            &[1, 2],
            |_| Ok(()),
            || {
                Err(ReviewError::Partial {
                    posted_comments: 0,
                    summary_posted: true,
                    source: FetchError::Network("offline".into()),
                })
            },
        );
        assert!(matches!(
            result,
            Err(ReviewError::Partial {
                posted_comments: 2,
                summary_posted: true,
                ..
            })
        ));
    }

    #[test]
    fn a_failure_with_nothing_sent_before_it_is_not_a_partial_one() {
        let offline = || Err(ReviewError::Failed(FetchError::Network("offline".into())));
        let none: [u8; 0] = [];
        let result = publish_steps(&none, |_| Ok(()), offline);
        assert!(matches!(result, Err(ReviewError::Failed(_))), "{result:?}");
        // With a comment posted first, the same failure leaves part of it sent.
        let result = publish_steps(&[1], |_| Ok(()), offline);
        assert!(matches!(
            result,
            Err(ReviewError::Partial {
                posted_comments: 1,
                summary_posted: false,
                ..
            })
        ));
    }
}
