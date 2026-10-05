use std::fmt;
use std::io::Write;
use std::time::Duration;

use super::{APP_PROPERTIES_PATH, http};

pub(crate) const SERVICE: &str = "slussa";

/// A Bitbucket HTTP access token. It cannot be printed by accident: `Debug`
/// hides it, and only `expose` hands it out, for the request header.
#[derive(Clone)]
pub struct Pat(String);

impl Pat {
    pub const fn new(token: String) -> Self {
        Self(token)
    }

    pub(super) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Pat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Pat(redacted)")
    }
}

/// What a token is stored under: the server it is for. An https or ssh remote of
/// a host is that host's name, as it has always been. An http remote has a name
/// of its own, so that a token stored for https is never sent over plain http,
/// as it would be when a directory's `origin` says http for the same host.
pub fn token_name(base_url: &str, host: &str) -> String {
    if base_url.starts_with("http://") {
        format!("http://{host}")
    } else {
        host.to_owned()
    }
}

pub fn load_pat(name: &str) -> Option<Pat> {
    let entry = keyring::Entry::new(SERVICE, name).ok()?;
    match entry.get_password() {
        Ok(pat) => Some(Pat::new(pat)),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            tracing::warn!("keyring lookup failed for {name}: {e}");
            None
        }
    }
}

fn save_pat(name: &str, pat: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
    entry.set_password(pat).map_err(|e| e.to_string())
}

pub fn login(host: &str, base_url: &str) -> Result<(), String> {
    println!("Detected host: {host}\n\n{}\n", token_setup_hint(base_url));
    if base_url.starts_with("http://") {
        println!(
            "Warning: {base_url} is not https, so the token is sent unencrypted. \
             Use an https remote if the server has one.\n"
        );
    }

    print!("HTTP access token: ");
    std::io::stdout().flush().ok();
    let pat = rpassword::read_password().map_err(|e| format!("couldn't read token: {e}"))?;
    let pat = pat.trim();
    if pat.is_empty() {
        return Err("no token entered.".into());
    }

    println!("Validating...");
    validate_pat(base_url, pat).map_err(|e| e.to_string())?;
    save_pat(&token_name(base_url, host), pat)?;
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
    let response = http::build_client(Duration::from_secs(10), http::Redirects::Refuse)
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
        && let Some(msg) = v
            .pointer("/errors/0/message")
            .and_then(serde_json::Value::as_str)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::bitbucket_dc::remote::base_url;

    /// What a token is stored under for an `origin` remote of that host.
    fn named(remote: &str, host: &str) -> String {
        token_name(&base_url(remote, host), host)
    }

    #[test]
    fn https_and_ssh_remotes_of_a_host_share_the_token_stored_under_its_name() {
        assert_eq!(named("https://bb.corp/scm/P/r.git", "bb.corp"), "bb.corp");
        assert_eq!(named("git@bb.corp:P/r.git", "bb.corp"), "bb.corp");
        assert_eq!(
            named("ssh://git@bb.corp:7999/P/r.git", "bb.corp"),
            "bb.corp"
        );
        assert_eq!(
            named("https://bb.corp:8443/scm/P/r.git", "bb.corp:8443"),
            "bb.corp:8443"
        );
    }

    #[test]
    fn an_http_remote_has_a_name_of_its_own_so_an_https_token_never_goes_over_http() {
        let https = named("https://bb.corp/scm/P/r.git", "bb.corp");
        let http = named("http://bb.corp/scm/P/r.git", "bb.corp");
        assert_eq!(http, "http://bb.corp");
        assert_ne!(http, https);
        assert_eq!(
            named("http://localhost:7990/scm/P/r.git", "localhost:7990"),
            "http://localhost:7990"
        );
    }
}
