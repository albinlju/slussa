//! The provider boundary: one `Provider` per session, dispatching to GitHub
//! (through `gh`) or Bitbucket Data Center (REST).
//!
//! - Every method blocks and returns `FetchError`. `app::fetchers` runs them
//!   on `spawn_blocking`; nothing here touches UI state.
//! - Each provider keeps its request and payload types private and maps them
//!   to `domain` types.
//! - `capabilities()` says what the connected provider supports, so screens
//!   hide unsupported actions instead of letting them fail.
//! - Writes are not exactly-once. A timeout or lost response can follow a
//!   write the server accepted, and a Bitbucket review is several requests, so
//!   a failure can be `ReviewError::Partial`. Nothing retries
//!   automatically.

pub mod bitbucket_dc;
pub mod error;
pub mod github;
mod unified_diff;

pub use error::{FetchError, ReviewError};

#[cfg(test)]
mod transport_tests;

use crate::domain::{
    activity::Activity,
    ci::Build,
    comment::{CommentKey, ThreadHandle},
    commit::Commit,
    diff::Diff,
    pr::{MergeStrategy, Mergeability, PrBatch, PrGroup, PrInfo},
    review::{ReviewComment, ReviewVerdict},
    user::Username,
};

#[derive(Clone, Debug)]
pub enum Provider {
    GitHub,
    BitbucketDc(bitbucket_dc::Config),
}

impl Provider {
    /// One group of the repository's PRs: all the open ones, or a page of the
    /// merged or declined ones starting after `after`. The batch's `more` is the
    /// position to pass back for the next page, `None` when there is none.
    pub fn fetch_prs(&self, group: PrGroup, after: Option<&str>) -> Result<PrBatch, FetchError> {
        match self {
            Self::GitHub => github::fetch_prs(group, after),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_prs(c, group, after),
        }
    }

    pub fn fetch_commits(&self, pr_id: u64) -> Result<Vec<Commit>, FetchError> {
        match self {
            Self::GitHub => github::fetch_commits(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_commits(c, pr_id),
        }
    }

    pub fn fetch_diff(&self, pr_id: u64) -> Result<Diff, FetchError> {
        match self {
            Self::GitHub => github::fetch_diff(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_diff(c, pr_id),
        }
    }

    pub fn fetch_commit_diff(&self, oid: &str) -> Result<Diff, FetchError> {
        match self {
            Self::GitHub => github::fetch_commit_diff(oid),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_commit_diff(c, oid),
        }
    }

    pub fn fetch_builds(&self, pr_id: u64) -> Result<Vec<Build>, FetchError> {
        match self {
            Self::GitHub => github::fetch_builds(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_builds(c, pr_id),
        }
    }

    pub fn fetch_activity(&self, pr_id: u64) -> Result<Activity, FetchError> {
        match self {
            Self::GitHub => github::fetch_activity(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_activity(c, pr_id),
        }
    }

    pub fn fetch_mergeability(&self, pr_id: u64) -> Result<Mergeability, FetchError> {
        match self {
            Self::GitHub => github::fetch_mergeability(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_mergeability(c, pr_id),
        }
    }

    /// The description and labels of one PR, for a provider whose list omits them.
    pub fn fetch_info(&self, pr_id: u64) -> Result<PrInfo, FetchError> {
        match self {
            Self::GitHub => github::fetch_info(pr_id),
            Self::BitbucketDc(_) => Err(FetchError::Unsupported(
                "Bitbucket lists the description and labels with each PR.".into(),
            )),
        }
    }

    pub fn merge(&self, pr_id: u64, strategy: MergeStrategy) -> Result<(), FetchError> {
        if !self.capabilities().merge_strategies.contains(&strategy) {
            return Err(FetchError::Unsupported(
                "This merge strategy is not supported by the connected provider.".into(),
            ));
        }
        match self {
            Self::GitHub => github::merge(pr_id, strategy),
            Self::BitbucketDc(c) => bitbucket_dc::merge(c, pr_id),
        }
    }

    /// Decline (Bitbucket) / close (GitHub) the PR without merging.
    pub fn decline(&self, pr_id: u64) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::decline(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::decline(c, pr_id),
        }
    }

    /// Reopen a PR that was closed or declined without being merged.
    pub fn reopen(&self, pr_id: u64) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::reopen(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::reopen(c, pr_id),
        }
    }

    pub fn capabilities(&self) -> crate::domain::capabilities::Capabilities {
        use crate::domain::capabilities::{Capabilities, Feature, ReviewSubmission};
        let features: std::collections::HashSet<Feature> = [
            Feature::PrComments,
            Feature::InlineComments,
            Feature::Replies,
            Feature::EditComments,
            Feature::DeleteComments,
            Feature::ResolveThreads,
            Feature::Builds,
            Feature::Mergeability,
            Feature::ClosePr,
            Feature::ReopenPr,
        ]
        .into_iter()
        .collect();
        match self {
            Self::GitHub => Capabilities {
                features: features.into_iter().chain([Feature::PrInfo]).collect(),
                review_verdicts: vec![
                    ReviewVerdict::Approve,
                    ReviewVerdict::RequestChanges,
                    ReviewVerdict::Comment,
                ],
                own_pr_verdicts: vec![ReviewVerdict::Comment],
                review_submission: ReviewSubmission::AtomicSingleRevision,
                merge_strategies: vec![
                    MergeStrategy::Merge,
                    MergeStrategy::Squash,
                    MergeStrategy::Rebase,
                ],
            },
            Self::BitbucketDc(_) => Capabilities {
                features,
                review_verdicts: vec![
                    ReviewVerdict::Approve,
                    ReviewVerdict::RequestChanges,
                    ReviewVerdict::Comment,
                    ReviewVerdict::Unapprove,
                ],
                own_pr_verdicts: vec![ReviewVerdict::Comment],
                review_submission: ReviewSubmission::Sequential,
                merge_strategies: vec![MergeStrategy::Merge],
            },
        }
    }

    pub fn post_comment(&self, pr_id: u64, comment: &ReviewComment) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::post_comment(pr_id, comment),
            Self::BitbucketDc(c) => bitbucket_dc::post_comment(c, pr_id, comment),
        }
    }

    pub fn post_pr_comment(&self, pr_id: u64, body: &str) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::post_pr_comment(pr_id, body),
            Self::BitbucketDc(c) => bitbucket_dc::post_pr_comment(c, pr_id, body),
        }
    }

    pub fn reply_comment(&self, pr_id: u64, parent: u64, body: &str) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::reply_comment(pr_id, parent, body),
            Self::BitbucketDc(c) => bitbucket_dc::reply_comment(c, pr_id, parent, body),
        }
    }

    /// GitHub edits a review comment and a PR comment through different
    /// endpoints, so the key says which it is; Bitbucket uses one for both.
    pub fn edit_comment(
        &self,
        pr_id: u64,
        comment: CommentKey,
        body: &str,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::edit_comment(comment, body),
            Self::BitbucketDc(c) => bitbucket_dc::edit_comment(c, pr_id, comment.id, body),
        }
    }

    pub fn delete_comment(&self, pr_id: u64, comment: CommentKey) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::delete_comment(comment),
            Self::BitbucketDc(c) => bitbucket_dc::delete_comment(c, pr_id, comment.id),
        }
    }

    /// Resolve or reopen a thread, addressed the way this provider read it.
    pub fn set_thread_resolved(
        &self,
        pr_id: u64,
        thread: &ThreadHandle,
        resolved: bool,
    ) -> Result<(), FetchError> {
        match (self, thread) {
            (Self::GitHub, ThreadHandle::NodeId(id)) => github::set_thread_resolved(id, resolved),
            (Self::BitbucketDc(c), ThreadHandle::RootComment(id)) => {
                bitbucket_dc::set_thread_resolved(c, pr_id, *id, resolved)
            }
            // A handle from the other provider: threads are read and resolved
            // by the same one, so this is a thread it never returned.
            (Self::GitHub, ThreadHandle::RootComment(_))
            | (Self::BitbucketDc(_), ThreadHandle::NodeId(_)) => Err(FetchError::InvalidInput(
                "This thread cannot be resolved by this provider.".into(),
            )),
        }
    }

    /// Submit a verdict together with a batch of queued inline `comments` (the
    /// `v` review flow). With no comments this is the same as `submit_review`.
    pub fn submit_full_review(
        &self,
        pr_id: u64,
        verdict: ReviewVerdict,
        body: &str,
        user: &str,
        comments: &[ReviewComment],
    ) -> Result<(), ReviewError> {
        match self {
            // One request: it arrives whole or not at all.
            Self::GitHub => Ok(github::submit_full_review(pr_id, verdict, body, comments)?),
            Self::BitbucketDc(c) => {
                bitbucket_dc::submit_full_review(c, pr_id, verdict, body, user, comments)
            }
        }
    }

    pub fn current_user(&self) -> Result<Username, FetchError> {
        match self {
            Self::GitHub => github::current_user(),
            Self::BitbucketDc(c) => bitbucket_dc::current_user(c),
        }
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;
    use crate::domain::capabilities::ReviewSubmission;

    #[test]
    fn adapters_declare_different_reviews_and_reject_unsupported_operations() {
        let github = Provider::GitHub.capabilities();
        let bitbucket = Provider::BitbucketDc(bitbucket_dc::Config {
            repo: bitbucket_dc::RepoLocation {
                base_url: "https://example.invalid".into(),
                project_key: "TEST".into(),
                repo_slug: "test".into(),
            },
            pat: String::new(),
        });
        let bb = bitbucket.capabilities();
        assert!(!github.can_submit_verdict(ReviewVerdict::Unapprove, false));
        assert!(bb.can_submit_verdict(ReviewVerdict::Unapprove, false));
        assert_eq!(
            github.review_submission,
            ReviewSubmission::AtomicSingleRevision
        );
        assert_eq!(bb.review_submission, ReviewSubmission::Sequential);
        assert!(github.merge_strategies.contains(&MergeStrategy::Squash));
        assert_eq!(bb.merge_strategies, vec![MergeStrategy::Merge]);
        // These must reject locally, before trying authentication or the network.
        assert!(bitbucket.merge(1, MergeStrategy::Squash).is_err());
        assert!(github::submit_review(1, ReviewVerdict::Unapprove, "").is_err());
    }
}
