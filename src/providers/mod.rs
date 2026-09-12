pub mod bitbucket_dc;
pub mod error;
pub mod github;
mod unified_diff;

pub use error::FetchError;

use crate::domain::{
    activity::Activity,
    ci::Build,
    commit::Commit,
    diff::Diff,
    pr::{MergeStrategy, Mergeability, PullRequest},
    review::{ReviewComment, ReviewVerdict},
};

#[derive(Clone, Debug)]
pub enum Provider {
    GitHub,
    BitbucketDc(bitbucket_dc::Config),
}

impl Provider {
    pub fn fetch_prs(&self) -> Result<Vec<PullRequest>, FetchError> {
        match self {
            Self::GitHub => github::fetch_prs(),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_prs(c),
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

    pub fn merge(&self, pr_id: u64, strategy: MergeStrategy) -> Result<(), FetchError> {
        if !self.capabilities().merge_strategies.contains(&strategy) {
            return Err(FetchError::InvalidInput(
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

    pub fn capabilities(&self) -> crate::domain::capabilities::Capabilities {
        use crate::domain::capabilities::{Capabilities, Feature, ReviewSubmission};
        let features = [
            Feature::PrComments,
            Feature::InlineComments,
            Feature::Replies,
            Feature::EditComments,
            Feature::DeleteComments,
            Feature::ResolveThreads,
            Feature::Builds,
            Feature::Mergeability,
            Feature::ClosePr,
        ]
        .into_iter()
        .collect();
        match self {
            Self::GitHub => Capabilities {
                features,
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

    pub fn post_comment(
        &self,
        pr_id: u64,
        anchor: &crate::domain::review::CommentAnchor,
        body: &str,
    ) -> Result<(), FetchError> {
        let revision = anchor.revision.as_ref().ok_or_else(|| {
            FetchError::InvalidInput(
                "Reload the diff before commenting: its revision is unknown.".into(),
            )
        })?;
        match self {
            Self::GitHub => github::post_comment(
                pr_id,
                &anchor.path,
                anchor.line,
                anchor.removed,
                body,
                revision,
            ),
            Self::BitbucketDc(c) => bitbucket_dc::post_comment(
                c,
                pr_id,
                &anchor.path,
                anchor.line,
                anchor.removed,
                body,
                revision,
            ),
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

    /// `review` distinguishes a diff/line comment from a PR-level one — GitHub
    /// edits them via different endpoints; Bitbucket uses one for both.
    pub fn edit_comment(
        &self,
        pr_id: u64,
        comment_id: u64,
        review: bool,
        body: &str,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::edit_comment(comment_id, review, body),
            Self::BitbucketDc(c) => bitbucket_dc::edit_comment(c, pr_id, comment_id, body),
        }
    }

    pub fn delete_comment(
        &self,
        pr_id: u64,
        comment_id: u64,
        review: bool,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::delete_comment(comment_id, review),
            Self::BitbucketDc(c) => bitbucket_dc::delete_comment(c, pr_id, comment_id),
        }
    }

    /// Resolve/unresolve a thread. GitHub needs the GraphQL thread `node_id`;
    /// Bitbucket toggles the root comment's state via `comment_id`.
    pub fn set_thread_resolved(
        &self,
        pr_id: u64,
        node_id: Option<&str>,
        comment_id: Option<u64>,
        resolved: bool,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => match node_id {
                Some(id) => github::set_thread_resolved(id, resolved),
                None => Err(FetchError::InvalidInput(
                    "This thread cannot be resolved by this provider.".into(),
                )),
            },
            Self::BitbucketDc(c) => match comment_id {
                Some(id) => bitbucket_dc::set_thread_resolved(c, pr_id, id, resolved),
                None => Err(FetchError::InvalidInput(
                    "This thread cannot be resolved by this provider.".into(),
                )),
            },
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
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::submit_full_review(pr_id, verdict, body, comments),
            Self::BitbucketDc(c) => {
                bitbucket_dc::submit_full_review(c, pr_id, verdict, body, user, comments)
            }
        }
    }

    pub fn current_user(&self) -> Result<String, FetchError> {
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
