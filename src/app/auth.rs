use std::io::Write;

use crate::app::preflight::{parse_remote_host, read_origin_remote};
use crate::providers::bitbucket_dc;

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

pub fn run_login() -> Result<(), String> {
    let remote = read_origin_remote().map_err(|e| e.to_string())?;
    let host = parse_remote_host(&remote)
        .ok_or_else(|| format!("couldn't parse host from remote `{remote}`"))?;

    println!(
        "Detected host: {host}\n\n{}\n",
        bitbucket_dc::token_setup_hint(&host)
    );

    print!("HTTP access token: ");
    std::io::stdout().flush().ok();
    let pat = rpassword::read_password().map_err(|e| format!("couldn't read token: {e}"))?;
    let pat = pat.trim();
    if pat.is_empty() {
        return Err("no token entered.".into());
    }

    println!("Validating...");
    bitbucket_dc::validate_pat(&host, pat).map_err(|e| e.to_string())?;
    save_pat(&host, pat)?;
    println!("Logged in. Token stored in system keyring.");
    Ok(())
}

