mod app;
mod clients;
mod domain;
mod logging;
mod tui;

use std::process::{Command, ExitCode};

use crate::app::App;
use crate::app::preflight::{self, PreflightError};

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("tuipr: couldn't initialise logging: {err}");
    }

    if let Err(err) = ensure_ready() {
        tracing::error!("preflight failed: {err}");
        eprintln!("tuipr: {err}");
        return ExitCode::from(1);
    }
    tracing::info!("preflight passed, starting tui");

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

fn ensure_ready() -> Result<(), PreflightError> {
    match preflight::preflight() {
        Ok(_) => Ok(()),
        Err(PreflightError::GhNotAuthenticated { host }) => {
            eprintln!(
                "tuipr: not logged in to {host}. Launching `gh auth login` — \
                 follow the prompts and tuipr will continue afterwards.\n"
            );
            let status = Command::new("gh")
                .args(["auth", "login", "-h", &host])
                .status()
                .map_err(|_| PreflightError::GhMissing)?;
            if !status.success() {
                eprintln!("tuipr: `gh auth login` was cancelled or failed.\n");
                return Err(PreflightError::GhNotAuthenticated { host });
            }
            preflight::preflight().map(|_| ())
        }
        Err(other) => Err(other),
    }
}
