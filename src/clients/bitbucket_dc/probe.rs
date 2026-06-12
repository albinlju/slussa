//! Unauthenticated backend detection used at preflight: does `host` serve a
//! Bitbucket Data Center instance? Knowledge of which endpoint identifies a DC
//! instance lives here, with the rest of the DC protocol — preflight only routes.

use std::time::Duration;

use crate::clients::bitbucket_dc::{APP_PROPERTIES_PATH, http};

/// `Ok(true)` if `host` is a Bitbucket DC instance, `Ok(false)` if it answers
/// but isn't one, and `Err(reason)` if it can't be reached (network/DNS).
pub fn probe(host: &str) -> Result<bool, String> {
    let url = format!("https://{host}{APP_PROPERTIES_PATH}");
    tracing::debug!("probing {url} for bitbucket dc");
    let response = http::build_client(Duration::from_secs(5))
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Ok(false);
    }
    let body: serde_json::Value = response.json().map_err(|e| e.to_string())?;
    // DC's application-properties endpoint includes `displayName: "Bitbucket"`.
    Ok(body
        .get("displayName")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.to_lowercase().contains("bitbucket")))
}
