use std::sync::OnceLock;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::de::DeserializeOwned;

use crate::providers::error::FetchError;

pub(super) fn build_client(timeout: Duration) -> reqwest::Result<Client> {
    Client::builder()
        .timeout(timeout)
        .user_agent(concat!("tuipr/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn client() -> Result<&'static Client, FetchError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let built = build_client(Duration::from_secs(20))
        .map_err(|e| FetchError::Network(format!("http client build failed: {e}")))?;
    Ok(CLIENT.get_or_init(|| built))
}

pub(super) fn get_json<T: DeserializeOwned>(
    base_url: &str,
    path: &str,
    pat: &str,
) -> Result<T, FetchError> {
    let url = format!("{base_url}{path}");
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
        let host = base_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        tracing::warn!("auth failed on {host} ({status})");
        return Err(FetchError::NotAuthenticated { host });
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

pub(super) fn current_user(base_url: &str, path: &str, pat: &str) -> Result<String, FetchError> {
    let url = format!("{base_url}{path}");
    tracing::debug!("GET {url} (whoami)");
    let response = client()?
        .get(&url)
        .bearer_auth(pat)
        .send()
        .map_err(|e| {
            tracing::warn!("http send failed: {e}");
            FetchError::Network(e.to_string())
        })?;
    // Bitbucket DC stamps the authenticated account on every response as
    // X-AUSERNAME; unauthenticated requests get "anonymous".
    response
        .headers()
        .get("X-AUSERNAME")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .filter(|u| !u.is_empty() && u != "anonymous")
        .ok_or_else(|| FetchError::ParseFailed("no X-AUSERNAME header".to_owned()))
}

pub(super) fn post_json<B: serde::Serialize>(
    base_url: &str,
    path: &str,
    pat: &str,
    body: &B,
) -> Result<(), FetchError> {
    let url = format!("{base_url}{path}");
    tracing::debug!("POST {url}");
    let response = client()?
        .post(&url)
        .bearer_auth(pat)
        .header("Accept", "application/json")
        .json(body)
        .send()
        .map_err(|e| {
            tracing::warn!("http send failed: {e}");
            FetchError::Network(e.to_string())
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let host = base_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        tracing::warn!("auth failed on {host} ({status})");
        return Err(FetchError::NotAuthenticated { host });
    }
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        tracing::warn!("http {} on {url}: {body}", status.as_u16());
        return Err(FetchError::HttpFailed {
            status: status.as_u16(),
            body,
        });
    }
    Ok(())
}
