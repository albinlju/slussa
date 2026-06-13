use std::fmt;
use std::time::Duration;

use crate::clients::bitbucket_dc::{APP_PROPERTIES_PATH, http};

#[derive(Debug)]
pub enum PatError {
    Rejected { status: u16, detail: String },
    ServerError { status: u16, detail: String },
    Unreachable(String),
}

impl fmt::Display for PatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { status, detail } => write!(
                f,
                "token rejected by server ({status}). {detail}\n\
                 Check that you pasted the whole token and it has Repository Read."
            ),
            Self::ServerError { status, detail } => {
                write!(f, "server returned http {status} — {detail}")
            }
            Self::Unreachable(reason) => write!(f, "couldn't reach server: {reason}"),
        }
    }
}

pub fn token_setup_hint(host: &str) -> String {
    format!(
        "1. Generate a HTTP access token:\n   \
            https://{host}/plugins/servlet/access-tokens/users/{{username}}/manage\n\
         2. Required permissions: PROJECT_READ, REPO_READ"
    )
}

pub fn validate_pat(host: &str, pat: &str) -> Result<(), PatError> {
    let url = format!("https://{host}{APP_PROPERTIES_PATH}");
    let response = http::build_client(Duration::from_secs(10))
        .map_err(|e| PatError::Unreachable(e.to_string()))?
        .get(&url)
        .bearer_auth(pat)
        .send()
        .map_err(|e| PatError::Unreachable(e.to_string()))?;

    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let detail = server_message(&response.text().unwrap_or_default());
    let code = status.as_u16();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        Err(PatError::Rejected { status: code, detail })
    } else {
        Err(PatError::ServerError { status: code, detail })
    }
}

fn server_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(msg) = v["errors"][0]["message"].as_str()
    {
        return msg.to_string();
    }
    let snippet: String = body.trim().chars().take(160).collect();
    if snippet.is_empty() {
        "(no response body)".to_string()
    } else {
        snippet
    }
}
