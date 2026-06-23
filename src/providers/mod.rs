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
    pr::{Mergeability, MergeStrategy, PullRequest},
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

    /// Merge strategies to offer in the picker. GitHub supports all three;
    /// Bitbucket DC merges with the repo's configured strategy, so just one entry.
    pub fn merge_strategies(&self) -> Vec<MergeStrategy> {
        match self {
            Self::GitHub => vec![
                MergeStrategy::Merge,
                MergeStrategy::Squash,
                MergeStrategy::Rebase,
            ],
            Self::BitbucketDc(_) => vec![MergeStrategy::Merge],
        }
    }

    pub fn post_comment(
        &self,
        pr_id: u64,
        path: &str,
        line: usize,
        removed: bool,
        body: &str,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::post_comment(pr_id, path, line, removed, body),
            Self::BitbucketDc(c) => bitbucket_dc::post_comment(c, pr_id, path, line, removed, body),
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
                None => Ok(()),
            },
            Self::BitbucketDc(c) => match comment_id {
                Some(id) => bitbucket_dc::set_thread_resolved(c, pr_id, id, resolved),
                None => Ok(()),
            },
        }
    }

    /// Submit a review verdict (+ optional summary `body`). `user` is the current
    /// user's slug — Bitbucket needs it for the participant endpoint; GitHub
    /// ignores it.
    pub fn submit_review(
        &self,
        pr_id: u64,
        verdict: ReviewVerdict,
        body: &str,
        user: &str,
    ) -> Result<(), FetchError> {
        match self {
            Self::GitHub => github::submit_review(pr_id, verdict, body),
            Self::BitbucketDc(c) => bitbucket_dc::submit_review(c, pr_id, verdict, body, user),
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

    /// Whether withdrawing one's own approval is supported. GitHub reviews are
    /// immutable, so it isn't offered there.
    pub fn can_unapprove(&self) -> bool {
        matches!(self, Self::BitbucketDc(_))
    }

    pub fn current_user(&self) -> Result<String, FetchError> {
        match self {
            Self::GitHub => github::current_user(),
            Self::BitbucketDc(c) => bitbucket_dc::current_user(c),
        }
    }
}
