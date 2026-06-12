mod app;
mod cli;
mod clients;
mod domain;
mod logging;
mod tui;

use std::process::ExitCode;

use crate::app::App;
use crate::clients::Backend;

fn main() -> ExitCode {
    if let Err(err) = logging::init() {
        eprintln!("tuipr: couldn't initialise logging: {err}");
    }

    match cli::dispatch(std::env::args().collect()) {
        cli::Dispatch::Done(code) => code,
        cli::Dispatch::RunTui(backend) => run_tui(backend),
    }
}

fn run_tui(backend: Backend) -> ExitCode {
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
