//! `b` on the Builds tab runs the failed builds again.

use super::support::*;
use crate::{
    domain::ci::{Build, BuildState},
    tui::app::store::Operation,
};

fn builds(app: &mut App, states: &[BuildState]) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.builds = LoadState::Loaded(
            states
                .iter()
                .map(|state| Build {
                    name: "ci".into(),
                    state: *state,
                    duration_ms: None,
                })
                .collect(),
        );
    }
}

#[tokio::test]
async fn b_runs_the_failed_builds_again() {
    let mut app = app();
    detail(&mut app, DetailTab::Builds);
    builds(&mut app, &[BuildState::Successful, BuildState::Failed]);
    press(&mut app, KeyCode::Char('b'));
    assert_eq!(
        app.state.store.operations.get(&PrId(42)),
        Some(&Operation::RerunBuilds)
    );
}

#[tokio::test]
async fn b_does_nothing_without_a_failed_build_or_off_the_builds_tab() {
    let mut app = app();
    detail(&mut app, DetailTab::Builds);
    builds(&mut app, &[BuildState::Successful]);
    press(&mut app, KeyCode::Char('b'));
    assert!(app.state.store.operations.is_empty());

    builds(&mut app, &[BuildState::Failed]);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('b'));
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn b_also_runs_a_cancelled_build_again() {
    let mut app = app();
    detail(&mut app, DetailTab::Builds);
    builds(&mut app, &[BuildState::Successful, BuildState::Cancelled]);
    press(&mut app, KeyCode::Char('b'));
    assert_eq!(
        app.state.store.operations.get(&PrId(42)),
        Some(&Operation::RerunBuilds)
    );
}
