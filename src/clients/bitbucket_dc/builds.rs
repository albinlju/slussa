//! The Builds tab. Bitbucket DC attaches build statuses to *commits*, not to
//! PRs directly, so we first read the PR to find its source branch's latest
//! commit, then pull the build statuses reported against that commit.
//!
//! Endpoints:
//! - `GET .../pull-requests/{id}`            → `fromRef.latestCommit`
//! - `GET .../commits/{commitId}/builds`     → paged build statuses
//!
//! A build's `state` is one of SUCCESSFUL / FAILED / INPROGRESS / CANCELLED /
//! UNKNOWN. `duration` (ms) and `name` are optional and only present on newer
//! instances / well-behaved CI integrations.

use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::ci::{Build, BuildState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbPrDetail {
    from_ref: BbFromRef,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbFromRef {
    #[serde(default)]
    latest_commit: String,
}

#[derive(Debug, Deserialize)]
struct PagedBuilds {
    #[serde(default)]
    values: Vec<BbBuild>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbBuild {
    #[serde(default)]
    key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    duration: Option<u64>,
}

pub fn fetch_builds(config: &Config, pr_id: u64) -> Result<Vec<Build>, FetchError> {
    let commit = latest_source_commit(config, pr_id)?;
    if commit.is_empty() {
        return Ok(Vec::new());
    }

    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/commits/{commit}/builds?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: PagedBuilds = get_json(&config.repo.host, &path, &config.pat)?;
    Ok(page.values.into_iter().map(map_build).collect())
}

fn latest_source_commit(config: &Config, pr_id: u64) -> Result<String, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}",
        config.repo.project_key, config.repo.repo_slug
    );
    let pr: BbPrDetail = get_json(&config.repo.host, &path, &config.pat)?;
    Ok(pr.from_ref.latest_commit)
}

fn map_build(b: BbBuild) -> Build {
    let state = match b.state.to_ascii_uppercase().as_str() {
        "SUCCESSFUL" => BuildState::Successful,
        "FAILED" => BuildState::Failed,
        "INPROGRESS" => BuildState::InProgress,
        "CANCELLED" => BuildState::Cancelled,
        _ => BuildState::Unknown,
    };
    // Fall back to the key when no display name is set — the key is required,
    // so the row is never blank.
    let name = b
        .name
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| b.key.clone());
    Build {
        key: b.key,
        name,
        state,
        description: b.description,
        url: b.url,
        duration_ms: b.duration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_states_and_falls_back_to_key_for_name() {
        let json = r#"{ "values": [
            { "key": "build", "name": "build", "state": "SUCCESSFUL", "duration": 72000 },
            { "key": "deploy", "state": "INPROGRESS" },
            { "key": "lint", "name": "", "state": "failed" },
            { "key": "weird", "name": "weird", "state": "BOGUS" }
        ]}"#;
        let page: PagedBuilds = serde_json::from_str(json).unwrap();
        let builds: Vec<Build> = page.values.into_iter().map(map_build).collect();

        assert_eq!(builds[0].state, BuildState::Successful);
        assert_eq!(builds[0].duration_ms, Some(72000));
        // Missing name falls back to the (required) key.
        assert_eq!(builds[1].name, "deploy");
        assert_eq!(builds[1].state, BuildState::InProgress);
        // Empty name also falls back; state match is case-insensitive.
        assert_eq!(builds[2].name, "lint");
        assert_eq!(builds[2].state, BuildState::Failed);
        // Unrecognised states degrade to Unknown rather than erroring.
        assert_eq!(builds[3].state, BuildState::Unknown);
    }
}
