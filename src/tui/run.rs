//! Starts the interactive program: the runtime, the application and the terminal.

use std::process::ExitCode;

use super::{
    app::{App, TerminalGuard},
    ui,
};
use crate::{
    config,
    domain::{self, pr::PrId},
    session::Session,
};

pub fn run(session: Session, open: Option<PrId>) -> ExitCode {
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
    ui::theme::init(config.theme.as_deref());

    let result = rt.block_on(async move {
        let mut app = match App::open(session) {
            Ok(app) => app.opening(open),
            Err(err) => {
                eprintln!("slussa: {err}");
                return ExitCode::from(1);
            }
        };
        app.state.ui.list.sort = ui::screens::pr_list::Sort::from_config(config.sort.as_deref());
        let (markers, blank) = domain::authorship::AiMarkers::from_config(&config.ai.markers);
        if blank > 0 {
            tracing::warn!("ignoring {blank} blank entries in `ai.markers` in the config");
        }
        app.state.store.set_ai_markers(markers);
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
