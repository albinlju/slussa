//! Bitbucket Data Center returns a plain unified-diff at the `/diff`
//! endpoint when asked for `text/plain` — same shape `git diff` outputs and
//! identical to what we already parse for GitHub. Reuse the GitHub parser
//! to avoid duplicating that logic.

use super::Config;
use crate::clients::bitbucket_dc::http::get_text;
use crate::clients::error::FetchError;
use crate::clients::unified_diff;
use crate::domain::diff::Diff;

pub fn fetch_diff(config: &Config, pr_id: u64) -> Result<Diff, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/diff",
        config.repo.project_key, config.repo.repo_slug
    );
    let text = get_text(&config.repo.host, &path, &config.pat)?;
    Ok(unified_diff::parse(&text))
}
