pub mod bitbucket_dc;
pub mod error;
pub mod github;
mod unified_diff;

pub use error::FetchError;

use crate::domain::{
    comment::{Comment, ReviewThread},
    commit::Commit,
    diff::Diff,
    pr::PullRequest,
};

/// Provider dispatch — one variant per supported backend. App holds a
/// `Backend` constructed at startup from preflight; every fetch hops through
/// here so future Bitbucket arms slot in as new match arms without touching
/// caller sites.
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
            // DC's "inline comment" model maps to ReviewThreads but the
            // wiring isn't built yet — surfacing an empty list here lets the
            // Overview tab render without "(failed to load)" until we add it.
            Self::BitbucketDc(_) => Ok(Vec::new()),
        }
    }
}
