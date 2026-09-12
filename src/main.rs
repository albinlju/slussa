mod app;
mod cli;
mod config;
mod domain;
mod git_url;
mod logging;
mod providers;
mod tui;

use std::process::ExitCode;

use crate::app::App;
use crate::providers::Provider;

fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("tuipr: couldn't initialise logging: {err}");
    }

    match cli::dispatch(std::env::args().collect()) {
        cli::Dispatch::Done(code) => code,
        cli::Dispatch::RunTui(provider) => run_tui(provider),
    }
}

fn run_tui(provider: Provider) -> ExitCode {
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

    let current_user = provider.current_user().unwrap_or_default();

    let result = rt.block_on(async move {
        let mut app = App::new(provider, current_user);
        if let Err(err) = app.enable_drafts() {
            eprintln!("tuipr: {err}");
            return ExitCode::from(1);
        }
        let mut terminal = ratatui::init();
        let result =
            match crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste) {
                Ok(()) => app.run(&mut terminal).await,
                Err(err) => Err(err),
            };
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste);
        ratatui::restore();
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("tuipr: {err}");
                ExitCode::from(1)
            }
        }
    });
    // Returning from the UI must not wait for blocking provider workers.
    // Quitting does not imply that an in-flight server write was cancelled.
    rt.shutdown_background();
    result
}
