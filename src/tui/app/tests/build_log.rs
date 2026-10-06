//! `enter` on a build in the Builds tab reads its log, once.

use super::support::*;
use crate::{
    domain::{
        build_log::BuildLog,
        ci::{Build, BuildState, JobId},
    },
    tui::app::store::{FetchKey, PrResource},
};

const KEY: FetchKey = FetchKey::Pr(PrResource::BuildLog(JobId(99)), PrId(42));

fn app_on_a_failed_build() -> App {
    let mut app = app();
    detail(&mut app, DetailTab::Builds);
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.builds = LoadState::Loaded(vec![Build {
            name: "test".into(),
            state: BuildState::Failed,
            duration_ms: None,
            log: Some(JobId(99)),
        }]);
    }
    app
}

fn log_state(app: &App) -> Option<&LoadState<BuildLog>> {
    app.state
        .store
        .cache
        .details
        .get(&PrId(42))?
        .build_logs
        .get(&JobId(99))
}

#[tokio::test(flavor = "current_thread")]
async fn enter_reads_the_log_and_the_result_is_kept_for_the_next_time() {
    let mut app = app_on_a_failed_build();
    press(&mut app, KeyCode::Enter);
    assert!(app.state.store.fetches.contains(&KEY));
    assert!(matches!(log_state(&app), Some(LoadState::Loading)));

    app.apply_result(TaskResult::Read(Read::BuildLog(
        PrId(42),
        JobId(99),
        Ok(BuildLog::parse("##[error]boom")),
    )));
    assert!(!app.state.store.fetches.contains(&KEY));
    assert!(matches!(log_state(&app), Some(LoadState::Loaded(_))));

    // Back to the builds and in again: what was read is not read again.
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Enter);
    assert!(!app.state.store.fetches.contains(&KEY));
}

#[tokio::test(flavor = "current_thread")]
async fn a_log_that_failed_is_read_again_the_next_time_it_is_opened() {
    let mut app = app_on_a_failed_build();
    press(&mut app, KeyCode::Enter);
    app.apply_result(TaskResult::Read(Read::BuildLog(
        PrId(42),
        JobId(99),
        Err(failed("offline")),
    )));
    assert!(matches!(log_state(&app), Some(LoadState::Failed(_))));
    assert!(!app.state.store.fetches.contains(&KEY));

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Enter);
    assert!(app.state.store.fetches.contains(&KEY));
}
