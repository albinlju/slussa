use std::io::IsTerminal;
use std::process::ExitCode;

use crate::providers::github;
use crate::session::{self, Session, preflight::PreflightError};

mod auth;

pub enum Dispatch {
    Done(ExitCode),
    RunTui(Session),
}

pub fn dispatch(mut args: Vec<String>) -> Dispatch {
    if let Some(pos) = args.iter().position(|a| a == "-C") {
        let Some(dir) = args.get(pos + 1).cloned() else {
            eprintln!("slussa: `-C` requires a directory argument.");
            return Dispatch::Done(ExitCode::from(2));
        };
        args.drain(pos..=pos + 1);
        if let Err(err) = std::env::set_current_dir(&dir) {
            eprintln!("slussa: couldn't chdir to {dir}: {err}");
            return Dispatch::Done(ExitCode::from(1));
        }
        tracing::info!("changed working directory to {dir}");
    }

    match args.get(1).map(String::as_str) {
        Some("auth") => return Dispatch::Done(auth::run(args.get(2..).unwrap_or_default())),
        Some("--help" | "-h") => {
            print_help();
            return Dispatch::Done(ExitCode::SUCCESS);
        }
        Some("--version" | "-V") => {
            println!("slussa {}", env!("CARGO_PKG_VERSION"));
            return Dispatch::Done(ExitCode::SUCCESS);
        }
        Some(other) => {
            eprintln!("slussa: unknown command `{other}`. Try `slussa --help`.");
            return Dispatch::Done(ExitCode::from(2));
        }
        None => {}
    }

    // Before anything touches the network: the TUI needs a terminal on both
    // ends, and a caller without one (an agent, a pipe, cron) is told which
    // commands do not need it.
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        eprintln!(
            "slussa: the PR browser needs a terminal, and stdin or stdout is not one.\n\
             Run it in a terminal; `slussa --help` lists the commands that print and exit."
        );
        return Dispatch::Done(ExitCode::from(2));
    }

    match connect() {
        Ok(session) => {
            tracing::info!("preflight passed, starting tui");
            Dispatch::RunTui(session)
        }
        Err(err) => {
            tracing::error!("preflight failed: {err}");
            eprintln!("slussa: {err}");
            Dispatch::Done(ExitCode::from(1))
        }
    }
}

fn print_help() {
    println!(
        "slussa — terminal UI for GitHub and Bitbucket Data Center pull requests\n\n\
         Usage:\n  \
         slussa                       Open the PR browser for the current repo.\n  \
         slussa -C <dir> [...]        Run as if started in <dir> (matches git/cargo -C).\n  \
         slussa auth login            Store a Bitbucket Data Center PAT for the current repo's host.\n  \
         slussa --version             Show the version.\n  \
         slussa --help                Show this message.\n"
    );
}

fn connect() -> Result<Session, PreflightError> {
    match session::connect() {
        Ok(session) => Ok(session),
        Err(PreflightError::GhNotAuthenticated { host }) => {
            eprintln!(
                "slussa: not logged in to {host}. Launching `gh auth login` — \
                 follow the prompts and slussa will continue afterwards.\n"
            );
            match github::auth::launch_login(&host) {
                Ok(true) => session::connect(),
                Ok(false) => {
                    eprintln!("slussa: `gh auth login` was cancelled or failed.\n");
                    Err(PreflightError::GhNotAuthenticated { host })
                }
                Err(_) => Err(PreflightError::GhMissing),
            }
        }
        Err(other) => Err(other),
    }
}
