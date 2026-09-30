use std::fmt;
use std::io::Write;
use std::time::Duration;

use super::{APP_PROPERTIES_PATH, http};

pub(crate) const SERVICE: &str = "tuipr";

pub fn load_pat(host: &str) -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, host).ok()?;
    match entry.get_password() {
        Ok(pat) => Some(pat),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            tracing::warn!("keyring lookup failed for {host}: {e}");
            None
        }
    }
}

fn save_pat(host: &str, pat: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, host).map_err(|e| e.to_string())?;
    entry.set_password(pat).map_err(|e| e.to_string())
}

pub fn login(host: &str, base_url: &str) -> Result<(), String> {
    println!("Detected host: {host}\n\n{}\n", token_setup_hint(base_url));

    print!("HTTP access token: ");
    std::io::stdout().flush().ok();
    let pat = rpassword::read_password().map_err(|e| format!("couldn't read token: {e}"))?;
    let pat = pat.trim();
    if pat.is_empty() {
        return Err("no token entered.".into());
    }

    println!("Validating...");
    validate_pat(base_url, pat).map_err(|e| e.to_string())?;
    save_pat(host, pat)?;
    println!("Logged in. Token stored in system keyring.");
    Ok(())
}

#[derive(Debug)]
enum PatError {
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

fn token_setup_hint(base_url: &str) -> String {
    format!(
        "1. Generate a HTTP access token:\n   \
            {base_url}/plugins/servlet/access-tokens/users/{{username}}/manage\n\
         2. Required permissions: PROJECT_READ, REPO_READ"
    )
}

fn validate_pat(base_url: &str, pat: &str) -> Result<(), PatError> {
    let url = format!("{base_url}{APP_PROPERTIES_PATH}");
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
        Err(PatError::Rejected {
            status: code,
            detail,
        })
    } else {
        Err(PatError::ServerError {
            status: code,
            detail,
        })
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
