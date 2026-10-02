//! The tickets a read and a write need, and what a failed write leaves behind.

use super::support::*;
use crate::{
    domain::capabilities::Feature,
    tui::app::store::{FetchKey, Operation, Store},
};

#[test]
fn a_read_gets_a_ticket_once_and_only_for_what_the_provider_has() {
    // The default capabilities offer none of the optional resources.
    let mut store = Store::default();
    assert!(store.begin_fetch(FetchKey::Builds(PrId(1))).is_none());
    assert!(!store.start_loading(&FetchKey::Builds(PrId(1))));
    assert!(store.fetches.is_empty());

    let ticket = store.begin_fetch(FetchKey::Diff(PrId(1))).unwrap();
    assert_eq!(ticket.key(), &FetchKey::Diff(PrId(1)));
    assert!(
        store.begin_fetch(FetchKey::Diff(PrId(1))).is_none(),
        "a read of it is already running"
    );
    assert!(store.begin_fetch(FetchKey::Diff(PrId(2))).is_some());

    store.capabilities.features.insert(Feature::Builds);
    assert!(store.begin_fetch(FetchKey::Builds(PrId(1))).is_some());
}

#[test]
fn loading_starts_once_and_leaves_loaded_data_alone() {
    let mut store = Store::default();
    let key = FetchKey::CommitDiff(PrId(1), "abc".into());
    assert!(store.start_loading(&key));
    assert!(!store.start_loading(&key), "already loading");

    let data = store.cache.details.get_mut(&PrId(1)).unwrap();
    data.commit_diffs
        .insert("abc".into(), LoadState::Failed(failed("offline")));
    assert!(store.start_loading(&key), "a failed read is tried again");

    let data = store.cache.details.get_mut(&PrId(1)).unwrap();
    data.commits = LoadState::Loaded(Vec::new());
    assert!(!store.start_loading(&FetchKey::Commits(PrId(1))));
    assert!(matches!(data_commits(&store), LoadState::Loaded(_)));
}

fn data_commits(store: &Store) -> &LoadState<Vec<crate::domain::commit::Commit>> {
    &store.cache.details[&PrId(1)].commits
}

#[test]
fn a_pr_takes_one_write_at_a_time() {
    let mut store = Store::default();
    let ticket = store.begin_write(PrId(7), Operation::Merge).unwrap();
    assert_eq!(
        (ticket.pr_id(), ticket.operation()),
        (PrId(7), Operation::Merge)
    );
    assert!(store.begin_write(PrId(7), Operation::Comment).is_none());
    assert!(store.begin_write(PrId(8), Operation::Comment).is_some());
    assert_eq!(store.operations[&PrId(7)], Operation::Merge);
}

#[test]
fn only_a_write_that_may_have_arrived_is_marked_uncertain() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    let mut fail = |error: FetchError| {
        app.state
            .store
            .operations
            .insert(PrId(42), Operation::Merge);
        finish_write(&mut app, PrId(42), Err(error.into()));
        (
            app.state.detail_view().error().map(str::to_owned),
            app.state.store.uncertain_submissions.contains(&PrId(42)),
        )
    };

    // Refused before it left: there is nothing to check on the server.
    assert_eq!(
        fail(FetchError::Unsupported("no such strategy".into())),
        (Some("no such strategy".into()), false)
    );
    // The server said no.
    let refused = FetchError::HttpFailed {
        status: 409,
        body: r#"{"errors":[{"message":"out of date"}]}"#.into(),
    };
    assert_eq!(fail(refused), (Some("out of date".into()), false));
    // No answer: it may have gone through.
    let (message, uncertain) = fail(FetchError::Timeout);
    assert!(message.is_some_and(|message| message.contains("timed out")));
    assert!(uncertain);
}

#[test]
fn a_read_names_the_resource_it_is_of() {
    assert_eq!(
        Read::CommitDiff(PrId(3), "abc".into(), Err(failed("offline"))).key(),
        FetchKey::CommitDiff(PrId(3), "abc".into())
    );
    let read = Read::Builds(PrId(3), Ok(Vec::new()));
    assert_eq!(read.key(), FetchKey::Builds(PrId(3)));
    assert!(read.failure().is_none());
    assert!(
        Read::Info(PrId(3), Err(failed("offline")))
            .failure()
            .is_some()
    );
}
