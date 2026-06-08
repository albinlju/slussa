mod app;
mod clients;
mod domain;
mod logging;
mod tui;

use std::process::{Command, ExitCode};

use crate::app::App;
use crate::app::preflight::{self, PreflightError};
use crate::clients::Backend;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("tuipr: couldn't initialise logging: {err}");
    }

    let mut args: Vec<String> = std::env::args().collect();
    match pop_chdir_flag(&mut args) {
        Ok(Some(dir)) => {
            if let Err(err) = std::env::set_current_dir(&dir) {
                eprintln!("tuipr: couldn't chdir to {dir}: {err}");
                return ExitCode::from(1);
            }
            tracing::info!("changed working directory to {dir}");
        }
        Ok(None) => {}
        Err(msg) => {
            eprintln!("tuipr: {msg}");
            return ExitCode::from(2);
        }
    }

    match args.get(1).map(String::as_str) {
        Some("auth") => return run_auth(&args[2..]),
        Some("--help" | "-h") => {
            print_help();
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let backend = match ensure_ready() {
        Ok(b) => b,
        Err(err) => {
            tracing::error!("preflight failed: {err}");
            eprintln!("tuipr: {err}");
            return ExitCode::from(1);
        }
    };
    tracing::info!("preflight passed, starting tui");

    let app = App::new(backend);
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

fn print_help() {
    println!(
        "tuipr — terminal UI for GitHub and Bitbucket Data Center pull requests\n\n\
         Usage:\n  \
         tuipr                       Open the PR browser for the current repo.\n  \
         tuipr -C <dir> [...]        Run as if started in <dir> (matches git/cargo -C).\n  \
         tuipr auth login            Store a Bitbucket Data Center PAT for the current repo's host.\n  \
         tuipr --help                Show this message.\n"
    );
}

/// Strip a `-C <dir>` flag out of `args` if present. Mirrors git/cargo
/// semantics: it must appear before any subcommand and consumes both the
/// flag and its argument. Returns `Err` for `-C` without a following dir.
fn pop_chdir_flag(args: &mut Vec<String>) -> Result<Option<String>, String> {
    for i in 1..args.len() {
        if args[i] == "-C" {
            if i + 1 >= args.len() {
                return Err("`-C` requires a directory argument.".into());
            }
            let dir = args.remove(i + 1);
            args.remove(i);
            return Ok(Some(dir));
        }
    }
    Ok(None)
}

fn run_auth(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("login") | None => match app::auth::run_login() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("tuipr: {err}");
                ExitCode::from(1)
            }
        },
        Some(other) => {
            eprintln!("tuipr: unknown auth subcommand `{other}`. Try `tuipr auth login`.");
            ExitCode::from(2)
        }
    }
}

fn ensure_ready() -> Result<Backend, PreflightError> {
    match preflight::preflight() {
        Ok(backend) => Ok(backend),
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
            preflight::preflight()
        }
        Err(other) => Err(other),
    }
}
