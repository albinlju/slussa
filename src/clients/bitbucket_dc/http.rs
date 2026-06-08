//! Shared HTTP helpers for Bitbucket Data Center REST v1 calls.

use std::time::Duration;

use reqwest::blocking::Client;
use serde::de::DeserializeOwned;

use crate::clients::error::FetchError;

pub(super) fn client() -> Result<Client, FetchError> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("tuipr/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| FetchError::Network(format!("http client build failed: {e}")))
}

pub(super) fn get_json<T: DeserializeOwned>(
    host: &str,
    path: &str,
    pat: &str,
) -> Result<T, FetchError> {
    let url = format!("{host}{path}");
    tracing::debug!("GET {url}");
    let response = client()?
        .get(&url)
        .bearer_auth(pat)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| {
            tracing::warn!("http send failed: {e}");
            FetchError::Network(e.to_string())
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let host_str = host
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        tracing::warn!("auth failed on {host_str} ({status})");
        return Err(FetchError::NotAuthenticated { host: host_str });
    }
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        tracing::warn!("http {} on {url}: {body}", status.as_u16());
        return Err(FetchError::HttpFailed {
            status: status.as_u16(),
            body,
        });
    }

    response.json::<T>().map_err(|e| {
        tracing::warn!("json parse failed on {url}: {e}");
        FetchError::ParseFailed(e.to_string())
    })
}

pub(super) fn get_text(host: &str, path: &str, pat: &str) -> Result<String, FetchError> {
    let url = format!("{host}{path}");
    tracing::debug!("GET {url}");
    let response = client()?
        .get(&url)
        .bearer_auth(pat)
        .send()
        .map_err(|e| {
            tracing::warn!("http send failed: {e}");
            FetchError::Network(e.to_string())
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let host_str = host
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        return Err(FetchError::NotAuthenticated { host: host_str });
    }
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(FetchError::HttpFailed {
            status: status.as_u16(),
            body,
        });
    }

    response.text().map_err(|e| FetchError::Network(e.to_string()))
}
