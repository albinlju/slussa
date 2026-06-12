//! GitHub readiness checks. tuipr drives GitHub entirely through the `gh` CLI,
//! so "is the backend ready" means "is gh installed and logged in" — that's gh
//! knowledge, and it lives here with the rest of the GitHub client rather than
//! in the preflight/bootstrap layer. (The data-fetching gh calls go through
//! `cli::run_gh`; these are status/interactive calls with different semantics.)

use std::process::Command;

/// Is the `gh` CLI installed and runnable?
pub fn is_installed() -> bool {
    Command::new("gh").arg("--version").output().is_ok()
}

/// Is `gh` authenticated for `host`? A non-zero exit means "not logged in",
/// which is a normal answer here rather than an error.
pub fn is_authenticated(host: &str) -> bool {
    Command::new("gh")
        .args(["auth", "status", "-h", host])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Launch interactive `gh auth login` for `host`, inheriting the terminal so the
/// user can complete the flow. `Ok(true)` if gh reported success.
pub fn launch_login(host: &str) -> std::io::Result<bool> {
    Command::new("gh")
        .args(["auth", "login", "-h", host])
        .status()
        .map(|status| status.success())
}
