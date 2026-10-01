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

use crate::app::App;
use crate::providers::Provider;

fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("slussa: couldn't initialise logging: {err}");
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
            eprintln!("slussa: couldn't start runtime: {err}");
            return ExitCode::from(1);
        }
    };

    // Drafts are stored per account, and "is this mine?" needs one too. Without
    // it the reason is shown here instead of a later, unrelated-looking failure.
    let current_user = match provider.current_user() {
        Ok(user) => user,
        Err(err) => {
            tracing::error!("couldn't identify the account: {err}");
            eprintln!(
                "slussa: couldn't identify the logged-in account: {}",
                err.user_message()
            );
            return ExitCode::from(1);
        }
    };

    let result = rt.block_on(async move {
        let mut app = App::new(provider, current_user);
        app.state.ui.list.sort =
            tui::screens::pr_list::Sort::from_config(config::load().sort.as_deref());
        if let Err(err) = app.enable_drafts() {
            eprintln!("slussa: {err}");
            return ExitCode::from(1);
        }
        let mut terminal = match ratatui::try_init() {
            Ok(terminal) => terminal,
            Err(err) => {
                // `try_init` turns raw mode on before the steps that can still
                // fail, so hand the terminal back before saying why.
                ratatui::restore();
                eprintln!("slussa: couldn't start the terminal UI: {err}");
                return ExitCode::from(1);
            }
        };
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
