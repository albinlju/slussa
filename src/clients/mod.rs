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

/// A PR's activity feed, parsed once and split three ways: general discussion
/// comments, lifecycle events, and inline review threads. Both backends produce
/// it from a single fetch — Bitbucket from one `/activities` call, GitHub from
/// one bundling call — mirroring how `structured` projects the diff payload.
#[derive(Debug, Default, Clone)]
pub struct ActivityBundle {
    pub comments: Vec<Comment>,
    pub events: Vec<TimelineEvent>,
    pub threads: Vec<ReviewThread>,
}

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

    pub fn fetch_builds(&self, pr_id: u64) -> Result<Vec<Build>, FetchError> {
        match self {
            Self::GitHub => github::fetch_builds(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_builds(c, pr_id),
        }
    }

    /// Comments, events, and inline threads in one shot. See [`ActivityBundle`].
    pub fn fetch_activity(&self, pr_id: u64) -> Result<ActivityBundle, FetchError> {
        match self {
            Self::GitHub => github::fetch_activity(pr_id),
            Self::BitbucketDc(c) => bitbucket_dc::fetch_activity(c, pr_id),
        }
    }
}
