mod cli;
mod config;
#[cfg(test)]
mod doc_contract;
mod domain;
mod git_url;
mod local;
mod logging;
mod private_file;
mod providers;
mod session;
#[cfg(test)]
mod test_support;
mod tui;

use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("slussa: couldn't initialise logging: {err}");
    }

    match cli::dispatch(std::env::args().collect()) {
        cli::Dispatch::Done(code) => code,
        cli::Dispatch::RunTui(session) => tui::run(session),
    }
}
