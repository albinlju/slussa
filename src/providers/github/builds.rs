use crate::{
    domain::{
        ci::{Build, BuildState},
        pr::PrId,
    },
    providers::FetchError,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct Check {
    name: String,
    status: String,
    conclusion: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}
#[derive(Deserialize)]
struct Status {
    context: String,
    state: String,
}

pub fn fetch_builds(pr_number: PrId) -> Result<Vec<Build>, FetchError> {
    let head = super::comments::head_sha(pr_number)?;
    let checks: Vec<Check> = pages(
        &format!("repos/{{owner}}/{{repo}}/commits/{head}/check-runs?per_page=100"),
        "check_runs",
    )?;
    let statuses: Vec<Status> = pages(
        &format!("repos/{{owner}}/{{repo}}/commits/{head}/status?per_page=100"),
        "statuses",
    )?;
    let mut builds: Vec<Build> = checks
        .into_iter()
        .map(|c| Build {
            name: c.name,
            state: state(c.conclusion.as_deref().unwrap_or(&c.status)),
            duration_ms: c
                .started_at
                .zip(c.completed_at)
                .map(|(a, b)| b.signed_duration_since(a).num_milliseconds().max(0) as u64),
        })
        .collect();
    builds.extend(statuses.into_iter().map(|s| Build {
        name: s.context,
        state: state(&s.state),
        duration_ms: None,
    }));
    Ok(builds)
}

/// Run again the jobs that failed, in each workflow run of the PR's head that
/// did not succeed. Checks that are not GitHub Actions cannot be run from here.
pub fn rerun_failed(pr_number: PrId) -> Result<(), FetchError> {
    #[derive(serde::Deserialize)]
    struct Run {
        id: u64,
        #[serde(default)]
        conclusion: Option<String>,
    }
    let head = super::comments::head_sha(pr_number)?;
    let runs: Vec<Run> = pages(
        &format!("repos/{{owner}}/{{repo}}/actions/runs?head_sha={head}&per_page=100"),
        "workflow_runs",
    )?;
    let failed: Vec<u64> = runs
        .into_iter()
        .filter(|run| {
            matches!(
                run.conclusion.as_deref(),
                Some("failure" | "timed_out" | "cancelled")
            )
        })
        .map(|run| run.id)
        .collect();
    if failed.is_empty() {
        return Err(FetchError::Unsupported(
            "No failed GitHub Actions run to run again.".into(),
        ));
    }
    // Every run is tried, so that one refusal does not hide the runs that start.
    let total = failed.len();
    let mut done = 0;
    let mut first_error = None;
    for id in failed {
        let started = super::cli::run_gh(&[
            "api",
            "--method",
            "POST",
            &format!("repos/{{owner}}/{{repo}}/actions/runs/{id}/rerun-failed-jobs"),
        ]);
        match started {
            Ok(_) => done += 1,
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        None => Ok(()),
        Some(error) if done == 0 => Err(error),
        Some(error) => Err(FetchError::Partial {
            done,
            total,
            first: error.user_message(),
        }),
    }
}

fn pages<T: serde::de::DeserializeOwned>(path: &str, field: &str) -> Result<Vec<T>, FetchError> {
    let pages: Vec<serde_json::Value> =
        super::cli::run_gh_json(&["api", "--paginate", "--slurp", path])?;
    let mut items = Vec::new();
    for mut page in pages {
        // `page[field]` would panic on a page that is not an object.
        let list = page
            .get_mut(field)
            .map(serde_json::Value::take)
            .ok_or_else(|| FetchError::ParseFailed(format!("Missing {field}").into()))?;
        items.extend(
            serde_json::from_value::<Vec<T>>(list)
                .map_err(|e| FetchError::ParseFailed(e.into()))?,
        );
    }
    Ok(items)
}

fn state(value: &str) -> BuildState {
    match value {
        "success" | "neutral" | "skipped" => BuildState::Successful,
        "failure" | "error" | "timed_out" | "action_required" | "startup_failure" => {
            BuildState::Failed
        }
        "pending" | "queued" | "in_progress" | "waiting" | "requested" => BuildState::InProgress,
        "cancelled" | "stale" => BuildState::Cancelled,
        _ => BuildState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_checks_and_legacy_commit_statuses() {
        assert_eq!(state("queued"), BuildState::InProgress);
        assert_eq!(state("pending"), BuildState::InProgress);
        assert_eq!(state("timed_out"), BuildState::Failed);
        assert_eq!(state("success"), BuildState::Successful);
    }
}
