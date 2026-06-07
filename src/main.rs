mod app;
mod domain;
#[allow(dead_code)]
mod providers;
mod tui;

use std::process::{Command, ExitCode};

use crate::app::App;
use crate::app::preflight::{self, PreflightError};

#[tokio::main]
async fn main() -> ExitCode {
    // Run startup checks before touching the terminal — if anything's off
    // (no gh, not logged in, not a github repo) we'd rather either prompt
    // the user to fix it or print to stderr and exit, rather than panic
    // inside raw mode and leave the terminal broken.
    if let Err(err) = ensure_ready() {
        eprintln!("tuipr: {err}");
        return ExitCode::from(1);
    }

    let app = App::new();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("tuipr: {err}");
            ExitCode::from(1)
        }
    }
}

/// Run preflight; on `GhNotAuthenticated` proxy the user through
/// `gh auth login` directly instead of forcing them to do it themselves.
/// All other errors bubble up unchanged.
fn ensure_ready() -> Result<(), PreflightError> {
    match preflight::preflight() {
        Ok(_) => Ok(()),
        Err(PreflightError::GhNotAuthenticated { host }) => {
            eprintln!(
                "tuipr: not logged in to {host}. Launching `gh auth login` — \
                 follow the prompts and tuipr will continue afterwards.\n"
            );
            // Inherit stdin/stdout/stderr so the user sees gh's interactive
            // browser/device-code prompts as usual. `.status()` blocks until
            // gh exits.
            let status = Command::new("gh")
                .args(["auth", "login", "-h", &host])
                .status()
                .map_err(|_| PreflightError::GhMissing)?;
            if !status.success() {
                eprintln!("tuipr: `gh auth login` was cancelled or failed.\n");
                return Err(PreflightError::GhNotAuthenticated { host });
            }
            // Re-run the full preflight — if gh still reports not-authed
            // (e.g. user picked the wrong host) we surface that cleanly.
            preflight::preflight().map(|_| ())
        }
        Err(other) => Err(other),
    }
}
