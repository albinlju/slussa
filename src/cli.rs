use std::process::ExitCode;

use crate::app::preflight::{self, PreflightError};
use crate::app::remote;
use crate::providers::{Provider, bitbucket_dc, github};

pub enum Dispatch {
    Done(ExitCode),
    RunTui(Provider),
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
        Some("auth") => return Dispatch::Done(run_auth(&args[2..])),
        Some("keyring-test") => return Dispatch::Done(run_keyring_test()),
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

    match resolve_provider() {
        Ok(provider) => {
            tracing::info!("preflight passed, starting tui");
            Dispatch::RunTui(provider)
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

fn run_auth(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("login") | None => {
            let result = remote::origin_url()
                .map_err(|e| e.to_string())
                .and_then(|url| {
                    let host = remote::parse_host(&url)
                        .ok_or_else(|| format!("couldn't read a host from `{url}`"))?;
                    let base = bitbucket_dc::remote::base_url(&url, &host);
                    bitbucket_dc::auth::login(&host, &base)
                });
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("slussa: {err}");
                    ExitCode::from(1)
                }
            }
        }
        Some(other) => {
            eprintln!("slussa: unknown auth subcommand `{other}`. Try `slussa auth login`.");
            ExitCode::from(2)
        }
    }
}

fn resolve_provider() -> Result<Provider, PreflightError> {
    match preflight::run() {
        Ok(provider) => Ok(provider),
        Err(PreflightError::GhNotAuthenticated { host }) => {
            eprintln!(
                "slussa: not logged in to {host}. Launching `gh auth login` — \
                 follow the prompts and slussa will continue afterwards.\n"
            );
            match github::auth::launch_login(&host) {
                Ok(true) => preflight::run(),
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

fn run_keyring_test() -> ExitCode {
    use crate::providers::bitbucket_dc::auth::SERVICE;
    use keyring::Entry;

    const ACCOUNT: &str = "slussa-keyring-test.localhost";
    const SECRET: &str = "test-token-12345";

    println!("Keyring test (service='{SERVICE}', account='{ACCOUNT}')");

    let entry = match Entry::new(SERVICE, ACCOUNT) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("✗ Entry::new failed: {err}");
            return ExitCode::from(1);
        }
    };

    print!("  set_password... ");
    if let Err(err) = entry.set_password(SECRET) {
        eprintln!("✗ {err}");
        return ExitCode::from(1);
    }
    println!("ok");

    print!("  get_password... ");
    let got = match entry.get_password() {
        Ok(s) => {
            println!("ok");
            s
        }
        Err(err) => {
            eprintln!("✗ {err}");
            return ExitCode::from(1);
        }
    };

    if got != SECRET {
        eprintln!("✗ Round-trip mismatch: expected {SECRET:?}, got {got:?}");
        return ExitCode::from(1);
    }
    println!("  round-trip values match");

    print!("  delete_credential (cleanup)... ");
    if let Err(err) = entry.delete_credential() {
        println!("warn: cleanup failed: {err}");
    } else {
        println!("ok");
    }

    println!("\n✓ Keyring backend is working.");
    println!(
        "  Verify in Keychain Access (macOS) or with:\n  \
         security find-generic-password -s {SERVICE} -a {ACCOUNT}"
    );
    ExitCode::SUCCESS
}
