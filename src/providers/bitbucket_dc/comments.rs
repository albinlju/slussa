use super::{Config, http};
use crate::providers::error::FetchError;

pub fn post_comment(
    config: &Config,
    pr_id: u64,
    path: &str,
    line: usize,
    removed: bool,
    text: &str,
) -> Result<(), FetchError> {
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

pub fn reply_comment(config: &Config, pr_id: u64, parent: u64, text: &str) -> Result<(), FetchError> {
    let endpoint = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/comments",
        config.repo.project_key, config.repo.repo_slug,
    );
    let body = serde_json::json!({ "text": text, "parent": { "id": parent } });
    http::post_json(&config.repo.base_url, &endpoint, &config.pat, &body)
}
