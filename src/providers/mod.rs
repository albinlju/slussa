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

// A match on one of our own enums names every variant, so that adding one is a
// compile error wherever it has to be handled. Tests assert by a catch-all.
#![cfg_attr(not(test), warn(clippy::wildcard_enum_match_arm))]

pub mod bitbucket_dc;
pub mod error;
pub mod github;
mod unified_diff;
mod url_path;

pub use error::{FetchError, MergeError, ReviewError};
pub use github::GhRepo;

#[cfg(test)]
mod transport_tests;

use crate::domain::{
    activity::Activity,
    ci::Build,
    comment::{CommentId, CommentKey, ThreadHandle},
    commit::{Commit, CommitOid},
    diff::Diff,
    pr::{
        AutoMerge, DeletableBranch, MergeStrategy, Mergeability, PrBatch, PrGroup, PrId, PrInfo,
        PullRequest,
    },
    review::{Rerequest, ReviewComment, ReviewVerdict, ReviewedHead},
    user::Username,
};

#[derive(Clone, Debug)]
pub enum Provider {
    /// GitHub through `gh`, always acting on one repository.
    GitHub(GhRepo),
    BitbucketDc(bitbucket_dc::Config),
}

#[cfg(test)]
impl Provider {
    /// GitHub acting on a repository no test looks at.
    pub fn github_for_test() -> Self {
        Self::GitHub(GhRepo::for_test())
    }
}

impl Provider {
    /// One group of the repository's PRs: all the open ones, or a page of the
    /// merged or declined ones starting after `after`. The batch's `more` is the
    /// position to pass back for the next page, `None` when there is none.
    pub fn fetch_prs(&self, group: PrGroup, after: Option<&str>) -> Result<PrBatch, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_prs(repo, group, after),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_prs(c, group, after),
        }
    }

    /// One PR by its number, as the list holds it.
    pub fn fetch_pr(&self, pr_id: PrId) -> Result<PullRequest, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_pr(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_pr(c, pr_id),
        }
    }

    pub fn fetch_commits(&self, pr_id: PrId) -> Result<Vec<Commit>, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_commits(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_commits(c, pr_id),
        }
    }

    pub fn fetch_diff(&self, pr_id: PrId) -> Result<Diff, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_diff(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_diff(c, pr_id),
        }
    }

    pub fn fetch_commit_diff(&self, oid: &CommitOid) -> Result<Diff, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_commit_diff(repo, oid),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_commit_diff(c, oid),
        }
    }

    pub fn fetch_builds(&self, pr_id: PrId) -> Result<Vec<Build>, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_builds(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_builds(c, pr_id),
        }
    }

    pub fn fetch_activity(&self, pr_id: PrId) -> Result<Activity, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_activity(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_activity(c, pr_id),
        }
    }

    pub fn fetch_mergeability(&self, pr_id: PrId) -> Result<Mergeability, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_mergeability(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_mergeability(c, pr_id),
        }
    }

    /// The description and labels of one PR, for a provider whose list omits them.
    pub fn fetch_info(&self, pr_id: PrId) -> Result<PrInfo, FetchError> {
        match self {
            Self::GitHub(repo) => github::fetch_info(repo, pr_id),
            Self::BitbucketDc(_) => Err(FetchError::Unsupported(
                "Bitbucket lists the description and labels with each PR.".into(),
            )),
        }
    }

    /// Merge the PR if its branch is still at `head`, the commit that was read,
    /// and, with `delete`, then delete the branch it came from.
    pub fn merge(
        &self,
        pr_id: PrId,
        strategy: MergeStrategy,
        delete: Option<&DeletableBranch>,
        head: &ReviewedHead,
    ) -> Result<(), MergeError> {
        let caps = self.capabilities();
        if !caps.merge_strategies.contains(&strategy) {
            return Err(FetchError::Unsupported(
                "This merge strategy is not supported by the connected provider.".into(),
            )
            .into());
        }
        if delete.is_some() && !caps.supports(crate::domain::capabilities::Feature::DeleteBranch) {
            return Err(FetchError::Unsupported(
                "The connected provider does not delete the branch with the merge.".into(),
            )
            .into());
        }
        match self {
            Self::GitHub(repo) => github::merge(repo, pr_id, strategy, delete, head),
            Self::BitbucketDc(c) => Ok(bitbucket_dc::merge(c, pr_id, head)?),
        }
    }

    /// Merge by itself once the PR is ready, or stop doing so.
    pub fn set_auto_merge(&self, pr_id: PrId, change: &AutoMerge) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::set_auto_merge(repo, pr_id, change),
            Self::BitbucketDc(_) => Err(FetchError::Unsupported(
                "Bitbucket does not merge a PR by itself when it is ready.".into(),
            )),
        }
    }

    /// Run again the builds of the PR that failed.
    pub fn rerun_failed_builds(&self, pr_id: PrId) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::rerun_failed(repo, pr_id),
            Self::BitbucketDc(_) => Err(FetchError::Unsupported(
                "Bitbucket builds cannot be run again from here.".into(),
            )),
        }
    }

    /// Decline (Bitbucket) / close (GitHub) the PR without merging.
    pub fn decline(&self, pr_id: PrId) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::decline(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::decline(c, pr_id),
        }
    }

    /// Ask those who asked for changes to review again.
    pub fn rerequest_review(&self, pr_id: PrId, who: &Rerequest) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::rerequest(repo, pr_id, who),
            Self::BitbucketDc(_) => Err(FetchError::Unsupported(
                "Bitbucket does not ask for a review again from here.".into(),
            )),
        }
    }

    /// Reopen a PR that was closed or declined without being merged.
    pub fn reopen(&self, pr_id: PrId) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::reopen(repo, pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::reopen(c, pr_id),
        }
    }

    pub fn capabilities(&self) -> crate::domain::capabilities::Capabilities {
        use crate::domain::capabilities::{Capabilities, Feature, ReviewCaps, ReviewSubmission};
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
            Self::GitHub(_) => Capabilities {
                features: features
                    .into_iter()
                    .chain([
                        Feature::PrInfo,
                        Feature::AutoMerge,
                        Feature::RerunBuilds,
                        Feature::DeleteBranch,
                        Feature::RerequestReview,
                    ])
                    .collect(),
                review: Some(ReviewCaps {
                    verdicts: vec![
                        ReviewVerdict::Approve,
                        ReviewVerdict::RequestChanges,
                        ReviewVerdict::Comment,
                    ],
                    own_pr_verdicts: vec![ReviewVerdict::Comment],
                    submission: ReviewSubmission::AtomicSingleRevision,
                }),
                merge_strategies: vec![
                    MergeStrategy::Merge,
                    MergeStrategy::Squash,
                    MergeStrategy::Rebase,
                ],
            },
            Self::BitbucketDc(_) => Capabilities {
                features,
                review: Some(ReviewCaps {
                    verdicts: vec![
                        ReviewVerdict::Approve,
                        ReviewVerdict::RequestChanges,
                        ReviewVerdict::Comment,
                        ReviewVerdict::Unapprove,
                    ],
                    own_pr_verdicts: vec![ReviewVerdict::Comment],
                    submission: ReviewSubmission::Sequential,
                }),
                merge_strategies: vec![MergeStrategy::Merge],
            },
        }
    }

    pub fn post_comment(&self, pr_id: PrId, comment: &ReviewComment) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::post_comment(repo, pr_id, comment),
            Self::BitbucketDc(c) => bitbucket_dc::post_comment(c, pr_id, comment),
        }
    }

    pub fn post_pr_comment(&self, pr_id: PrId, body: &str) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::post_pr_comment(repo, pr_id, body),
            Self::BitbucketDc(c) => bitbucket_dc::post_pr_comment(c, pr_id, body),
        }
    }

    pub fn reply_comment(
        &self,
        pr_id: PrId,
        parent: CommentId,
        body: &str,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::reply_comment(repo, pr_id, parent, body),
            Self::BitbucketDc(c) => bitbucket_dc::reply_comment(c, pr_id, parent, body),
        }
    }

    /// GitHub edits a review comment and a PR comment through different
    /// endpoints, so the key says which it is; Bitbucket uses one for both.
    pub fn edit_comment(
        &self,
        pr_id: PrId,
        comment: CommentKey,
        body: &str,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::edit_comment(repo, comment, body),
            Self::BitbucketDc(c) => bitbucket_dc::edit_comment(c, pr_id, comment.id, body),
        }
    }

    pub fn delete_comment(&self, pr_id: PrId, comment: CommentKey) -> Result<(), FetchError> {
        match self {
            Self::GitHub(repo) => github::delete_comment(repo, comment),
            Self::BitbucketDc(c) => bitbucket_dc::delete_comment(c, pr_id, comment.id),
        }
    }

    /// Resolve or reopen a thread, addressed the way this provider read it.
    pub fn set_thread_resolved(
        &self,
        pr_id: PrId,
        thread: &ThreadHandle,
        resolved: bool,
    ) -> Result<(), FetchError> {
        match (self, thread) {
            (Self::GitHub(repo), ThreadHandle::NodeId(id)) => {
                github::set_thread_resolved(repo, id, resolved)
            }
            (Self::BitbucketDc(c), ThreadHandle::RootComment(id)) => {
                bitbucket_dc::set_thread_resolved(c, pr_id, *id, resolved)
            }
            // A handle from the other provider: threads are read and resolved
            // by the same one, so this is a thread it never returned.
            (Self::GitHub(_), ThreadHandle::RootComment(_))
            | (Self::BitbucketDc(_), ThreadHandle::NodeId(_)) => Err(FetchError::InvalidInput(
                "This thread cannot be resolved by this provider.".into(),
            )),
        }
    }

    /// Submit a verdict together with a batch of queued inline `comments` (the
    /// `v` review flow). With no comments this is the same as `submit_review`.
    pub fn submit_full_review(
        &self,
        pr_id: PrId,
        verdict: ReviewVerdict,
        body: &str,
        user: &str,
        comments: &[ReviewComment],
        head: &ReviewedHead,
    ) -> Result<(), ReviewError> {
        match self {
            // One request: it arrives whole or not at all.
            Self::GitHub(repo) => Ok(github::submit_full_review(
                repo, pr_id, verdict, body, comments, head,
            )?),
            Self::BitbucketDc(c) => {
                bitbucket_dc::submit_full_review(c, pr_id, verdict, body, user, comments, head)
            }
        }
    }

    pub fn current_user(&self) -> Result<Username, FetchError> {
        match self {
            Self::GitHub(repo) => github::current_user(repo),
            Self::BitbucketDc(c) => bitbucket_dc::current_user(c),
        }
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    fn read_head() -> ReviewedHead {
        ReviewedHead::of(None, Some("abc123")).expect("a listed head")
    }
    use crate::domain::capabilities::ReviewSubmission;

    #[test]
    fn adapters_declare_different_reviews_and_reject_unsupported_operations() {
        let github = Provider::github_for_test().capabilities();
        let bitbucket = Provider::BitbucketDc(bitbucket_dc::Config {
            repo: bitbucket_dc::RepoLocation {
                base_url: "https://example.invalid".into(),
                project_key: "TEST".into(),
                repo_slug: "test".into(),
            },
            pat: bitbucket_dc::auth::Pat::new(String::new()),
        });
        let bb = bitbucket.capabilities();
        assert!(!github.can_submit_verdict(ReviewVerdict::Unapprove, false));
        assert!(bb.can_submit_verdict(ReviewVerdict::Unapprove, false));
        assert_eq!(
            github.review.map(|review| review.submission),
            Some(ReviewSubmission::AtomicSingleRevision)
        );
        assert_eq!(
            bb.review.map(|review| review.submission),
            Some(ReviewSubmission::Sequential)
        );
        assert!(github.merge_strategies.contains(&MergeStrategy::Squash));
        assert_eq!(bb.merge_strategies, vec![MergeStrategy::Merge]);
        // These must reject locally, before trying authentication or the network.
        assert!(
            bitbucket
                .merge(PrId(1), MergeStrategy::Squash, None, &read_head())
                .is_err()
        );
        assert!(
            github::submit_review(
                &GhRepo::for_test(),
                PrId(1),
                ReviewVerdict::Unapprove,
                "",
                &read_head(),
            )
            .is_err()
        );
    }
}
