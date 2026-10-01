use serde::Deserialize;

use super::Config;
use crate::domain::ci::{Build, BuildState};
use crate::domain::pr::PrId;
use crate::providers::bitbucket_dc::http::{get_all, get_json};
use crate::providers::error::FetchError;

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

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct PagedBuilds {
    #[serde(default)]
    values: Vec<BbBuild>,
}

#[derive(Debug, Deserialize)]
struct BbBuild {
    #[serde(default)]
    key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    duration: Option<u64>,
}

pub fn fetch_builds(config: &Config, pr_id: PrId) -> Result<Vec<Build>, FetchError> {
    let commit = latest_source_commit(config, pr_id)?;
    if commit.is_empty() {
        return Ok(Vec::new());
    }

    let path = format!("/rest/build-status/1.0/commits/{commit}?limit=100");
    let values: Vec<BbBuild> = get_all(&config.repo.base_url, &path, &config.pat)?;
    Ok(values.into_iter().map(map_build).collect())
}

fn latest_source_commit(config: &Config, pr_id: PrId) -> Result<String, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}",
        config.repo.project_key, config.repo.repo_slug
    );
    let pr: BbPrDetail = get_json(&config.repo.base_url, &path, &config.pat)?;
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
    let name = b.name.filter(|n| !n.is_empty()).unwrap_or(b.key);
    Build {
        name,
        state,
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
        assert_eq!(builds[1].name, "deploy");
        assert_eq!(builds[1].state, BuildState::InProgress);
        assert_eq!(builds[2].name, "lint");
        assert_eq!(builds[2].state, BuildState::Failed);
        assert_eq!(builds[3].state, BuildState::Unknown);
    }
}
