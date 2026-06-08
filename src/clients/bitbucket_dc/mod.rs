//! Bitbucket Data Center client (self-hosted, REST v1).
//!
//! The HTTP entrypoint and per-resource fetchers live in submodules so we can
//! mirror `clients::github`'s shape. `RepoCoords` and `Config` are the values
//! that the `Backend::BitbucketDc` enum arm carries.

mod comments;
mod commits;
mod diff;
mod http;
mod prs;
pub mod remote;

pub use comments::fetch_comments;
pub use commits::fetch_commits;
pub use diff::fetch_diff;
pub use prs::fetch_prs;

/// Identifies a single repo on a Data Center instance — derived from `git
/// remote get-url origin` at preflight time.
#[derive(Clone, Debug)]
pub struct RepoCoords {
    /// Base URL like `https://bitbucket.kunden.se` (no trailing slash).
    pub host: String,
    /// Project key, uppercase (e.g. `PLAT`).
    pub project_key: String,
    /// Repository slug, lowercase (e.g. `payments-api`).
    pub repo_slug: String,
}

/// Everything `BitbucketDc` fetchers need: where the repo lives + the PAT to
/// authenticate against it.
#[derive(Clone, Debug)]
pub struct Config {
    pub repo: RepoCoords,
    pub pat: String,
}
