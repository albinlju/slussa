//! What the reader has looked at: a PR is seen when it is opened and while it
//! is on screen, so only what happens after that lights it up.

use super::support::*;
use crate::{
    domain::pr::{PrBatch, PrGroup},
    local::seen::SeenStorage,
    test_support::TempDir,
    tui::app::seen::SeenFile,
};
use chrono::Duration;

fn list(app: &App) -> Vec<crate::domain::pr::PullRequest> {
    app.state
        .store
        .cache
        .prs
        .loaded()
        .cloned()
        .unwrap_or_default()
}

fn unread(app: &App, id: u64) -> bool {
    list(app)
        .iter()
        .find(|pr| pr.id == PrId(id))
        .is_some_and(|pr| app.state.store.seen.is_unread(pr))
}

/// The list read again, with the PR updated `by` later than it was.
fn refresh_with_update(app: &mut App, id: u64, by: Duration) {
    let mut prs = list(app);
    for pr in prs.iter_mut().filter(|pr| pr.id == PrId(id)) {
        pr.updated += by;
    }
    app.apply_result(TaskResult::Read(Read::Prs {
        group: PrGroup::Open,
        after: None,
        result: Ok(PrBatch { prs, more: None }),
    }));
}

#[test]
fn a_pr_never_opened_is_not_unread_however_much_happens_to_it() {
    let mut app = app();
    refresh_with_update(&mut app, 42, Duration::hours(3));
    assert!(!unread(&app, 42));
}

#[test]
fn a_pr_is_seen_when_opened_and_unread_once_it_changes_after_the_reader_left() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    assert!(!unread(&app, 42), "opening it is looking at it");

    // It changes while the reader is looking at it: seen, as the reader sees it.
    refresh_with_update(&mut app, 42, Duration::hours(1));
    assert!(!unread(&app, 42), "a PR on screen is seen at its newest");

    // The reader goes back to the list; now a change is news.
    app.apply(Action::from(NavAction::Back));
    refresh_with_update(&mut app, 42, Duration::hours(2));
    assert!(unread(&app, 42));

    // Opening it again clears it.
    detail(&mut app, DetailTab::Overview);
    assert!(!unread(&app, 42));
}

#[test]
fn what_was_looked_at_is_written_when_it_changes_and_read_back_on_the_next_start() {
    let dir = TempDir::new("seen-app");
    let (storage, _) = SeenStorage::open(dir.path(), "scope".into()).unwrap();
    let mut app = app();
    app.seen_file = SeenFile::Disk(storage);

    detail(&mut app, DetailTab::Overview);
    assert!(!app.seen_dirty, "written at once, not left for later");

    // Another start: the file holds the look, and the same list marks the PR
    // once it has been updated since.
    app.seen_file = SeenFile::Unavailable;
    let (_storage, seen) = SeenStorage::open(dir.path(), "scope".into()).unwrap();
    assert_eq!(seen.len(), 1);
    let mut later = list(&app).remove(0);
    later.updated += Duration::minutes(5);
    assert!(seen.is_unread(&later));
}

#[test]
fn without_a_file_the_marks_last_the_run() {
    let mut app = app();
    assert!(matches!(app.seen_file, SeenFile::Unavailable));
    detail(&mut app, DetailTab::Overview);
    app.apply(Action::from(NavAction::Back));
    refresh_with_update(&mut app, 42, Duration::hours(1));
    assert!(unread(&app, 42), "kept in memory all the same");
}
