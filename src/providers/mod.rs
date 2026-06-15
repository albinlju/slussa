pub mod bitbucket_dc;
pub mod error;
pub mod github;
mod unified_diff;

pub use error::FetchError;

use crate::domain::{activity::Activity, ci::Build, commit::Commit, diff::Diff, pr::PullRequest};

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
}
