use std::time::Duration;

use crate::providers::bitbucket_dc::{APP_PROPERTIES_PATH, http};

pub fn is_instance(base_url: &str) -> Result<bool, String> {
    let url = format!("{base_url}{APP_PROPERTIES_PATH}");
    tracing::debug!("probing {url} for bitbucket dc instance");
    let response = http::build_client(Duration::from_secs(5), http::Redirects::Follow)
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Ok(false);
    }
    let body: serde_json::Value = response.json().map_err(|e| e.to_string())?;
    Ok(body
        .get("displayName")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.to_lowercase().contains("bitbucket")))
}
