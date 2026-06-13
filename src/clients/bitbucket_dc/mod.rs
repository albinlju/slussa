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

pub(super) const APP_PROPERTIES_PATH: &str = "/rest/api/1.0/application-properties";

#[derive(Clone, Debug)]
pub struct RepoCoords {
    pub base_url: String,
    pub project_key: String,
    pub repo_slug: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub repo: RepoCoords,
    pub pat: String,
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
