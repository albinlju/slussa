mod app;
mod clients;
mod domain;
mod logging;
mod tui;

use std::process::{Command, ExitCode};

use crate::app::App;
use crate::app::preflight::{self, PreflightError};
use crate::clients::Backend;

fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("tuipr: couldn't initialise logging: {err}");
    }

    let mut args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "-C") {
        let Some(dir) = args.get(pos + 1).cloned() else {
            eprintln!("tuipr: `-C` requires a directory argument.");
            return ExitCode::from(2);
        };
        args.drain(pos..=pos + 1);
        if let Err(err) = std::env::set_current_dir(&dir) {
            eprintln!("tuipr: couldn't chdir to {dir}: {err}");
            return ExitCode::from(1);
        }
        tracing::info!("changed working directory to {dir}");
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

    // Build a multi-thread runtime only here — main + preflight + auth stay
    // sync so blocking HTTP calls (reqwest::blocking) don't trip tokio's
    // nested-runtime guard. Only the TUI event loop needs async.
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("tuipr: couldn't start runtime: {err}");
            return ExitCode::from(1);
        }
    };

    rt.block_on(async move {
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
    })
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
