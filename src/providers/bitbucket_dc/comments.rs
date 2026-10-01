use serde::Deserialize;

use super::{Config, http};
use crate::providers::error::FetchError;

pub fn post_comment(
    config: &Config,
    pr_id: u64,
    path: &str,
    line: usize,
    removed: bool,
    text: &str,
    revision: &crate::domain::diff::DiffRevision,
) -> Result<(), FetchError> {
    if revision.base.is_none() {
        return Err(FetchError::InvalidInput(
            "The diff has no base revision. Reload it before commenting.".into(),
        ));
    }
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/comments",
        config.repo.project_key, config.repo.repo_slug,
    );
    let (line_type, file_type) = if removed {
        ("REMOVED", "FROM")
    } else {
        ("ADDED", "TO")
    };
    let body = serde_json::json!({
        "text": text,
        "anchor": {
            "fromHash": revision.base,
            "toHash": revision.head,
            "diffType": if revision.commit { "COMMIT" } else { "EFFECTIVE" },
            "path": path,
            "line": line,
            "lineType": line_type,
            "fileType": file_type,
        }
    });
    http::post_json(&config.repo.base_url, &endpoint, &config.pat, &body)
}

pub fn post_pr_comment(config: &Config, pr_id: u64, text: &str) -> Result<(), FetchError> {
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/comments",
        config.repo.project_key, config.repo.repo_slug,
    );
    let body = serde_json::json!({ "text": text });
    http::post_json(&config.repo.base_url, &endpoint, &config.pat, &body)
}

pub fn reply_comment(
    config: &Config,
    pr_id: u64,
    parent: u64,
    text: &str,
) -> Result<(), FetchError> {
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/comments",
        config.repo.project_key, config.repo.repo_slug,
    );
    let body = serde_json::json!({ "text": text, "parent": { "id": parent } });
    http::post_json(&config.repo.base_url, &endpoint, &config.pat, &body)
}

pub fn edit_comment(
    config: &Config,
    pr_id: u64,
    comment_id: u64,
    text: &str,
) -> Result<(), FetchError> {
    // Editing requires the current version (optimistic locking); fetch it here so
    // callers (and the domain) never have to carry it.
    let version = comment_version(config, pr_id, comment_id)?;
    let body = serde_json::json!({ "version": version, "text": text });
    http::put_json(
        &config.repo.base_url,
        &comment_path(config, pr_id, comment_id),
        &config.pat,
        &body,
    )
}

pub fn delete_comment(config: &Config, pr_id: u64, comment_id: u64) -> Result<(), FetchError> {
    let version = comment_version(config, pr_id, comment_id)?;
    let path = format!(
        "{}?version={version}",
        comment_path(config, pr_id, comment_id)
    );
    http::delete(&config.repo.base_url, &path, &config.pat)
}

pub fn set_thread_resolved(
    config: &Config,
    pr_id: u64,
    comment_id: u64,
    resolved: bool,
) -> Result<(), FetchError> {
    let version = comment_version(config, pr_id, comment_id)?;
    let state = if resolved { "RESOLVED" } else { "OPEN" };
    let body = serde_json::json!({ "version": version, "state": state });
    http::put_json(
        &config.repo.base_url,
        &comment_path(config, pr_id, comment_id),
        &config.pat,
        &body,
    )
}

fn comment_path(config: &Config, pr_id: u64, comment_id: u64) -> String {
    format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/comments/{comment_id}",
        config.repo.project_key, config.repo.repo_slug,
    )
}

fn comment_version(config: &Config, pr_id: u64, comment_id: u64) -> Result<u32, FetchError> {
    #[derive(Deserialize)]
    struct VersionOnly {
        version: u32,
    }
    let c: VersionOnly = http::get_json(
        &config.repo.base_url,
        &comment_path(config, pr_id, comment_id),
        &config.pat,
    )?;
    Ok(c.version)
}
