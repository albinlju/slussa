use std::io::Write;
use std::time::Duration;

use crate::app::preflight::{parse_remote_host, read_origin_remote};

const SERVICE: &str = "tuipr";

/// Look up a stored PAT for `host`. `None` means "not authenticated yet".
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

pub fn run_login() -> Result<(), String> {
    let remote = read_origin_remote().map_err(|e| e.to_string())?;
    let host = parse_remote_host(&remote)
        .ok_or_else(|| format!("couldn't parse host from remote `{remote}`"))?;

    println!(
        "Detected host: {host}\n\n\
         1. Generate a HTTP access token:\n\
            https://{host}/plugins/servlet/access-tokens/users/{{username}}/manage\n\
         2. Required permissions: PROJECT_READ, REPO_READ\n"
    );

    print!("HTTP access token: ");
    std::io::stdout().flush().ok();
    let pat = rpassword::read_password().map_err(|e| format!("couldn't read token: {e}"))?;
    let pat = pat.trim();
    if pat.is_empty() {
        return Err("no token entered.".into());
    }

    println!("Validating...");
    validate_pat(&host, pat)?;
    save_pat(&host, pat)?;
    println!("Logged in. Token stored in system keyring.");
    Ok(())
}

fn validate_pat(host: &str, pat: &str) -> Result<(), String> {
    let url = format!("https://{host}/rest/api/1.0/application-properties");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("tuipr/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(&url)
        .bearer_auth(pat)
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    // Bitbucket's error body usually says why (expired, wrong scope, …).
    let body = response.text().unwrap_or_default();
    let detail = server_message(&body);
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(format!(
            "token rejected by server ({}). {detail}\n\
             Check that you pasted the whole token and it has Repository Read.",
            status.as_u16()
        ));
    }
    Err(format!(
        "server returned http {} — {detail}",
        status.as_u16()
    ))
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

