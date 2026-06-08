pub mod bitbucket_dc;
pub mod error;
pub mod github;
mod unified_diff;

pub use error::FetchError;

use crate::domain::{
    ci::Build,
    comment::{Comment, ReviewThread},
    commit::Commit,
    diff::Diff,
    event::TimelineEvent,
    pr::PullRequest,
};

#[derive(Clone, Debug)]
pub enum Backend {
    /// GitHub via the `gh` CLI. No client state — gh handles auth/session.
    GitHub,
    /// Bitbucket Data Center via REST v1 with a PAT.
    BitbucketDc(bitbucket_dc::Config),
}

impl Backend {
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

    pub fn fetch_comments(&self, pr_id: u64) -> Result<Vec<Comment>, FetchError> {
        match self {
            Self::GitHub => github::fetch_comments(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_comments(c, pr_id),
        }
    }

    pub fn fetch_review_threads(&self, pr_id: u64) -> Result<Vec<ReviewThread>, FetchError> {
        match self {
            Self::GitHub => github::fetch_review_threads(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_review_threads(c, pr_id),
        }
    }

    pub fn fetch_builds(&self, pr_id: u64) -> Result<Vec<Build>, FetchError> {
        match self {
            Self::GitHub => github::fetch_builds(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_builds(c, pr_id),
        }
    }

    pub fn fetch_events(&self, pr_id: u64) -> Result<Vec<TimelineEvent>, FetchError> {
        match self {
            Self::GitHub => github::fetch_events(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_events(c, pr_id),
        }
    }
}
