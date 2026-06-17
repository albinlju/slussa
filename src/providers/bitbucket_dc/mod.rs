mod activities;
pub mod auth;
mod builds;
mod comments;
mod commits;
mod diff;
mod http;
mod json_diff;
mod probe;
mod prs;
pub mod remote;

use chrono::{DateTime, TimeZone, Utc};

use crate::providers::error::FetchError;

pub use activities::fetch as fetch_activity;
pub use builds::fetch_builds;
pub use comments::{
    delete_comment, edit_comment, post_comment, post_pr_comment, reply_comment,
    set_thread_resolved,
};
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use probe::is_instance;
pub use prs::fetch_prs;

pub(super) const APP_PROPERTIES_PATH: &str = "/rest/api/1.0/application-properties";

pub fn current_user(config: &Config) -> Result<String, FetchError> {
    http::current_user(&config.repo.base_url, APP_PROPERTIES_PATH, &config.pat)
}

pub fn approve(config: &Config, pr_id: u64, user: &str) -> Result<(), FetchError> {
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/participants/{user}",
        config.repo.project_key, config.repo.repo_slug,
    );
    let body = serde_json::json!({ "status": "APPROVED" });
    http::put_json(&config.repo.base_url, &endpoint, &config.pat, &body)
}

#[derive(Clone, Debug)]
pub struct RepoLocation {
    pub base_url: String,
    pub project_key: String,
    pub repo_slug: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub repo: RepoLocation,
    pub pat: String,
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}
