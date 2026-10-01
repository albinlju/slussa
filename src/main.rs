mod app;
mod cli;
mod config;
mod domain;
mod git_url;
mod logging;
mod providers;
#[cfg(test)]
mod test_support;
mod tui;

use std::process::ExitCode;

use ratatui::DefaultTerminal;

use crate::app::{App, preflight::Session};

fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("slussa: couldn't initialise logging: {err}");
    }

    match cli::dispatch(std::env::args().collect()) {
        cli::Dispatch::Done(code) => code,
        cli::Dispatch::RunTui(session) => run_tui(session),
    }
}

/// The terminal in TUI mode. Dropping it hands the terminal back as it was,
/// also when the event loop returns early or a panic unwinds past it.
struct TerminalGuard {
    terminal: DefaultTerminal,
}

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        // `try_init` turns raw mode on before the steps that can still fail, and
        // there is no guard yet to undo that, so a failure restores here.
        let terminal = ratatui::try_init().inspect_err(|_| ratatui::restore())?;
        let guard = Self { terminal };
        // If this fails the guard is dropped, which restores the terminal.
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste);
        ratatui::restore();
    }
}

fn run_tui(session: Session) -> ExitCode {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("slussa: couldn't start runtime: {err}");
            return ExitCode::from(1);
        }
    };

    let config = config::load();
    tui::theme::init(config.theme.as_deref());

    let result = rt.block_on(async move {
        let mut app = match App::open(session) {
            Ok(app) => app,
            Err(err) => {
                eprintln!("slussa: {err}");
                return ExitCode::from(1);
            }
        };
        app.state.ui.list.sort = tui::screens::pr_list::Sort::from_config(config.sort.as_deref());
        let mut guard = match TerminalGuard::enter() {
            Ok(guard) => guard,
            Err(err) => {
                eprintln!("slussa: couldn't start the terminal UI: {err}");
                return ExitCode::from(1);
            }
        };
        let result = app.run(&mut guard.terminal).await;
        // Back to the normal screen before anything is printed.
        drop(guard);
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("slussa: {err}");
                ExitCode::from(1)
            }
        }
    });
    // Returning from the UI must not wait for blocking provider workers.
    // Quitting does not imply that an in-flight server write was cancelled.
    rt.shutdown_background();
    result
}
