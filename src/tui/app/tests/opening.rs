//! `slussa 44`: the PR named when slussa was started is read for itself and
//! opened, whether or not the list holds it.

use super::support::*;
use crate::domain::pr::{PrBatch, PrGroup, PullRequest};

/// A PR the fixture's list does not hold.
fn elsewhere(app: &App, id: u64) -> PullRequest {
    let LoadState::Loaded(prs) = &app.state.store.cache.prs else {
        panic!("the fixture's list is loaded");
    };
    let mut pr = prs[0].clone();
    pr.id = PrId(id);
    pr.title = "An old change".into();
    pr
}

fn on_screen(app: &App) -> Option<PrId> {
    match app.state.screen {
        Screen::Detail { pr_id, .. } => Some(pr_id),
        Screen::List => None,
    }
}

fn ids(app: &App) -> Vec<u64> {
    app.state
        .store
        .cache
        .prs
        .loaded()
        .map_or_else(Vec::new, |prs| prs.iter().map(|pr| pr.id.0).collect())
}

/// slussa started with the PR's number: its screen at once, before anything is read.
fn started_on(app: &mut App, id: u64) {
    app.start_on = Some(PrId(id));
    app.start_opening();
}

#[tokio::test]
async fn the_pr_named_is_on_screen_before_it_is_read() {
    let mut app = app();
    app.state.screen = Screen::List;
    started_on(&mut app, 99);
    assert_eq!(on_screen(&app), Some(PrId(99)));
    assert!(!ids(&app).contains(&99));
}

#[tokio::test]
async fn a_pr_the_list_does_not_hold_is_added_to_it() {
    let mut app = app();
    started_on(&mut app, 99);
    let pr = elsewhere(&app, 99);
    app.apply_result(TaskResult::Read(Read::Pr(PrId(99), Ok(pr))));
    assert_eq!(on_screen(&app), Some(PrId(99)));
    assert!(
        ids(&app).contains(&99),
        "a PR on screen has to be in the list"
    );
    assert!(app.state.store.requested.is_none());
}

#[tokio::test]
async fn a_pr_the_list_holds_is_not_added_twice() {
    let mut app = app();
    let pr = {
        let LoadState::Loaded(prs) = &app.state.store.cache.prs else {
            panic!("the fixture's list is loaded");
        };
        prs[0].clone()
    };
    started_on(&mut app, pr.id.0);
    let before = ids(&app);
    app.apply_result(TaskResult::Read(Read::Pr(pr.id, Ok(pr.clone()))));
    assert_eq!(on_screen(&app), Some(pr.id));
    assert_eq!(ids(&app), before);
}

#[tokio::test]
async fn a_pr_read_before_the_list_waits_for_it() {
    let mut app = app();
    let pr = elsewhere(&app, 99);
    let other = elsewhere(&app, 7);
    app.state.store.cache.prs = LoadState::Loading;
    started_on(&mut app, 99);

    app.apply_result(TaskResult::Read(Read::Pr(PrId(99), Ok(pr))));
    assert!(
        app.state.store.requested.is_some(),
        "the list is not read yet"
    );

    // The list arrives, without the PR; now the PR is in it.
    app.apply_result(TaskResult::Read(Read::Prs {
        group: PrGroup::Open,
        after: None,
        result: Ok(PrBatch {
            prs: vec![other],
            more: None,
        }),
    }));
    assert_eq!(on_screen(&app), Some(PrId(99)));
    assert!(ids(&app).contains(&99) && ids(&app).contains(&7));
    assert!(app.state.store.requested.is_none());
}

#[tokio::test]
async fn a_pr_that_cannot_be_read_is_said_so_and_the_list_is_shown() {
    let mut app = app();
    started_on(&mut app, 99);
    app.apply_result(TaskResult::Read(Read::Pr(
        PrId(99),
        Err(failed("Could not resolve to a PullRequest")),
    )));
    assert_eq!(on_screen(&app), None);
    let notice = app.state.store.notice.as_ref().map(|n| n.message.clone());
    assert!(
        notice.is_some_and(|text| text.contains("#99") && text.contains("Couldn't open")),
        "{:?}",
        app.state.store.notice
    );
}
