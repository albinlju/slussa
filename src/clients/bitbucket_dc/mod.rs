mod activities;
mod builds;
mod commits;
mod diff;
mod http;
mod probe;
mod prs;
pub mod remote;
mod structured;
mod token;

use chrono::{DateTime, TimeZone, Utc};

pub use activities::fetch as fetch_activity;
pub use builds::fetch_builds;
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use probe::probe;
pub use prs::fetch_prs;
pub use token::{token_setup_hint, validate_pat};

/// Unauthenticated-readable endpoint used both to detect a DC instance (probe)
/// and to validate a PAT against it (token). One source for the path.
pub(super) const APP_PROPERTIES_PATH: &str = "/rest/api/1.0/application-properties";

/// Identifies a single repo on a Data Center instance — derived from `git
/// remote get-url origin` at preflight time.
#[derive(Clone, Debug)]
pub struct RepoCoords {
    /// Base URL like `https://bitbucket.kunden.se` (no trailing slash) —
    /// distinct from the bare hostname that `probe`/`token` take.
    pub base_url: String,
    /// Project key, uppercase (e.g. `PLAT`).
    pub project_key: String,
    /// Repository slug, lowercase (e.g. `payments-api`).
    pub repo_slug: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub repo: RepoCoords,
    pub pat: String,
}

/// Bitbucket DC timestamps are epoch milliseconds.
fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
