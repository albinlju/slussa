//! End-to-end flows through the real `App`, provider and fetchers, with a fake
//! `gh` underneath. These cover what the unit tests inject by hand: requests
//! really leave, results really return, and the store reacts to them.
//!
//! Each test holds the installed fake until its work has settled, because the
//! fake replaces `gh` for the whole process.

// The fake guard is deliberately held across awaits; the lint's suggestion to
// drop it early would let the next test replace `gh` mid-flow.
#![allow(clippy::significant_drop_tightening)]

use std::time::Duration;

use serde_json::json;

use super::{
    App,
    action::{Action, Command, ListAction},
    reviews::CommentTarget,
    store::{FetchKey, LoadState},
};
use crate::{
    providers::Provider,
    test_support::{FakeGh, gh_closed_pr, gh_list_page, gh_pr},
};

fn app() -> App {
    App::new(Provider::GitHub, "me".into())
}

/// Apply provider results until nothing is in flight.
async fn settle(app: &mut App) {
    while !app.state.store.fetches.is_empty() || !app.state.store.operations.is_empty() {
        let action = tokio::time::timeout(Duration::from_secs(10), app.action_rx.recv())
            .await
            .expect("a provider result within 10 seconds")
            .expect("the action channel stays open");
        app.apply(action);
    }
}

fn one_pr_page() -> String {
    gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], None)
}

/// The list makes two reads: the open PRs, and one page of recent closed ones.
const OPEN_QUERY: &str = "states: OPEN";
const CLOSED_QUERY: &str = "states: [MERGED, CLOSED]";

fn no_closed_prs() -> String {
    gh_list_page(&[], None)
}

/// Calls that read the open PRs, and calls that read the closed ones.
fn list_reads(calls: &[String]) -> (usize, usize) {
    let count = |needle: &str| calls.iter().filter(|call| call.contains(needle)).count();
    (count(OPEN_QUERY), count(CLOSED_QUERY))
}

/// An answer shaped like every connection the refetch touches, so each read
/// gets something parseable without modelling GitHub.
fn any_connection() -> String {
    let connection = json!({"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}});
    json!({"data": {
        "repository": {
            "connection": connection,
            "item": {"connection": connection},
            "pullRequest": {"mergeable": "MERGEABLE"}
        },
        "item": {"connection": connection}
    }})
    .to_string()
}

fn loaded_ids(app: &App) -> Vec<u64> {
    match &app.state.store.cache.prs {
        LoadState::Loaded(prs) => prs.iter().map(|pr| pr.id).collect(),
        other => panic!("PR list is not loaded: {other:?}"),
    }
}

#[tokio::test]
async fn list_loads_through_the_provider_and_deduplicates_a_repeat_request() {
    let gh = FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .on(CLOSED_QUERY, &no_closed_prs())
        .install();
    let mut app = app();

    app.spawn_load_prs();
    app.spawn_load_prs();
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1]);
    assert_eq!(
        list_reads(&gh.calls()),
        (1, 1),
        "a repeat request reuses the one in flight: {:?}",
        gh.calls()
    );
    assert!(app.state.store.refresh_failures.is_empty());
}

#[tokio::test]
async fn failed_refresh_keeps_the_list_then_recovers() {
    let gh = FakeGh::new()
        .once(OPEN_QUERY, &one_pr_page())
        .fail_once(OPEN_QUERY, 1, "gh: HTTP 502: Bad Gateway")
        .on(OPEN_QUERY, &one_pr_page())
        .on(CLOSED_QUERY, &no_closed_prs())
        .install();
    let mut app = app();

    app.spawn_load_prs();
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1]);

    app.spawn_load_prs();
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1], "stale data stays visible");
    assert!(
        app.state.store.refresh_failures.contains(&FetchKey::Prs),
        "the failure is recorded so the UI can say so"
    );

    app.spawn_load_prs();
    settle(&mut app).await;
    assert!(app.state.store.refresh_failures.is_empty());
    assert_eq!(
        list_reads(&gh.calls()),
        (3, 2),
        "a failed open read is not followed by a closed read: {:?}",
        gh.calls()
    );
}

#[tokio::test]
async fn first_load_failure_is_shown_as_failed_not_empty() {
    let _gh = FakeGh::new()
        .fail("pullRequests(", 1, "gh: HTTP 401: Bad credentials")
        .install();
    let mut app = app();

    app.spawn_load_prs();
    settle(&mut app).await;

    match &app.state.store.cache.prs {
        LoadState::Failed(message) => assert!(message.contains("Bad credentials"), "{message}"),
        other => panic!("expected a failed load, got {other:?}"),
    }
}

fn submit_pr_comment(app: &mut App, pr_id: u64, text: &str) {
    app.apply(Action::Command {
        pr_id,
        command: Command::SubmitComment {
            target: CommentTarget::Pr,
            text: text.into(),
        },
    });
}

#[tokio::test]
async fn a_comment_is_sent_once_and_followed_by_a_refetch() {
    let gh = FakeGh::new()
        .on("issues/1/comments", "{}")
        .on(OPEN_QUERY, &one_pr_page())
        .on(CLOSED_QUERY, &no_closed_prs())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();

    submit_pr_comment(&mut app, 1, "hello");
    assert!(
        app.state.store.operations.contains_key(&1),
        "the write is pending until the provider answers"
    );
    settle(&mut app).await;

    let calls = gh.calls();
    assert_eq!(
        calls[0],
        "api --method POST repos/{owner}/{repo}/issues/1/comments -f body=hello"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.contains("issues/1/comments"))
            .count(),
        1,
        "{calls:?}"
    );
    for (what, needle) in [
        ("the activity", "comments(first: 100"),
        ("the PR list", "states: OPEN"),
        ("mergeability", "mergeable"),
    ] {
        assert!(
            calls.iter().any(|c| c.contains(needle)),
            "{what} is refetched after the write: {calls:?}"
        );
    }
    assert!(app.state.store.errors.is_empty());
    assert!(app.state.store.uncertain_submissions.is_empty());
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_failed_comment_is_reported_on_its_pr_and_not_followed_by_a_refetch() {
    let gh = FakeGh::new()
        .fail(
            "issues/1/comments",
            1,
            "gh: HTTP 500: Internal Server Error",
        )
        .install();
    let mut app = app();

    submit_pr_comment(&mut app, 1, "hello");
    settle(&mut app).await;

    assert_eq!(gh.calls().len(), 1, "no refetch after a failed write");
    assert!(app.state.store.errors.contains_key(&1));
    assert!(
        app.state.store.uncertain_submissions.contains(&1),
        "an ambiguous failure is remembered so the user checks before retrying"
    );
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_second_write_to_the_same_pr_is_ignored_while_one_is_pending() {
    let gh = FakeGh::new()
        .on("issues/1/comments", "{}")
        .on(OPEN_QUERY, &one_pr_page())
        .on(CLOSED_QUERY, &no_closed_prs())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();

    submit_pr_comment(&mut app, 1, "first");
    submit_pr_comment(&mut app, 1, "second");
    settle(&mut app).await;

    let posts: Vec<_> = gh
        .calls()
        .into_iter()
        .filter(|c| c.contains("issues/1/comments"))
        .collect();
    assert_eq!(posts.len(), 1, "{posts:?}");
    assert!(posts[0].ends_with("body=first"));
}

// ---------------------------------------------------------------------------
// Older merged and declined PRs
// ---------------------------------------------------------------------------

const FIRST_CLOSED: &str = "first: 50, after: null";

/// A fake whose closed PRs span three reads: the first page (with a cursor),
/// then `after: "x"` (with another), then `after: "y"` (the end).
fn history() -> FakeGh {
    FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .on(
            FIRST_CLOSED,
            &gh_list_page(
                &[gh_closed_pr(3, "2026-08-01T10:00:00Z", "MERGED")],
                Some("x"),
            ),
        )
        .on(
            "after: \"x\"",
            &gh_list_page(
                &[gh_closed_pr(2, "2026-07-01T10:00:00Z", "CLOSED")],
                Some("y"),
            ),
        )
        .on(
            "after: \"y\"",
            &gh_list_page(&[gh_closed_pr(4, "2026-06-01T10:00:00Z", "MERGED")], None),
        )
}

fn older_reads(calls: &[String]) -> usize {
    calls
        .iter()
        .filter(|call| call.contains("after: \"x\"") || call.contains("after: \"y\""))
        .count()
}

async fn load_first_page(app: &mut App) {
    app.spawn_load_prs();
    settle(app).await;
}

#[tokio::test]
async fn older_prs_are_appended_until_the_history_ends() {
    let gh = history().install();
    let mut app = app();
    load_first_page(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3]);
    assert_eq!(app.state.store.older_cursor.as_deref(), Some("x"));

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2]);
    assert_eq!(app.state.store.older_cursor.as_deref(), Some("y"));
    assert_eq!(
        app.state.store.notice.as_ref().map(|n| n.message.as_str()),
        Some("Loaded 1 older PR")
    );

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2, 4]);
    assert_eq!(app.state.store.older_cursor, None, "the history has ended");

    let reads = older_reads(&gh.calls());
    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(
        older_reads(&gh.calls()),
        reads,
        "nothing is asked for once it ends"
    );
}

#[tokio::test]
async fn pressing_load_older_twice_makes_one_request() {
    let gh = history().install();
    let mut app = app();
    load_first_page(&mut app).await;

    app.apply(Action::List(ListAction::LoadOlder));
    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;

    assert_eq!(older_reads(&gh.calls()), 1, "{:?}", gh.calls());
    assert_eq!(loaded_ids(&app), vec![1, 3, 2]);
}

#[tokio::test]
async fn a_refresh_keeps_the_older_prs_already_loaded_and_the_place_reached() {
    let _gh = history().install();
    let mut app = app();
    load_first_page(&mut app).await;
    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2]);

    // The minute-by-minute refresh reads the first page again.
    load_first_page(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1, 3, 2], "PR 2 is not thrown away");
    assert_eq!(
        app.state.store.older_cursor.as_deref(),
        Some("y"),
        "and the place reached is kept, not reset to the first page's"
    );
}

#[tokio::test]
async fn a_failed_older_read_keeps_the_list_says_so_and_can_be_retried() {
    let gh = FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .on(
            FIRST_CLOSED,
            &gh_list_page(
                &[gh_closed_pr(3, "2026-08-01T10:00:00Z", "MERGED")],
                Some("x"),
            ),
        )
        .fail_once("after: \"x\"", 1, "gh: HTTP 502: Bad Gateway")
        .on(
            "after: \"x\"",
            &gh_list_page(&[gh_closed_pr(2, "2026-07-01T10:00:00Z", "CLOSED")], None),
        )
        .install();
    let mut app = app();
    load_first_page(&mut app).await;

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3], "the list is untouched");
    assert_eq!(app.state.store.older_cursor.as_deref(), Some("x"));
    let notice = app.state.store.notice.as_ref().unwrap();
    assert!(notice.error && notice.message.contains("Couldn't load older PRs"));
    assert!(
        app.state.store.refresh_failures.is_empty(),
        "a failed older read is not a refresh failure"
    );

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2]);
    assert_eq!(older_reads(&gh.calls()), 2);
}

// ---------------------------------------------------------------------------
// Reopening a PR
// ---------------------------------------------------------------------------

fn reopen(app: &mut App, pr_id: u64) {
    app.apply(Action::Command {
        pr_id,
        command: Command::Reopen,
    });
}

#[tokio::test]
async fn reopening_is_sent_once_reported_and_followed_by_a_refetch() {
    let gh = FakeGh::new()
        .on("pulls/1 -f state=open", "{}")
        .on(OPEN_QUERY, &one_pr_page())
        .on(CLOSED_QUERY, &no_closed_prs())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();

    reopen(&mut app, 1);
    assert!(
        app.state.store.operations.contains_key(&1),
        "pending until the provider answers"
    );
    settle(&mut app).await;

    let calls = gh.calls();
    assert_eq!(
        calls[0],
        "api --method PATCH repos/{owner}/{repo}/pulls/1 -f state=open"
    );
    assert_eq!(calls.iter().filter(|c| c.contains("state=open")).count(), 1);
    assert!(
        calls.iter().any(|c| c.contains(OPEN_QUERY)),
        "the list is refetched so the PR shows as open: {calls:?}"
    );
    assert_eq!(
        app.state.store.notice.as_ref().map(|n| n.message.as_str()),
        Some("PR #1 · reopened")
    );
    assert!(app.state.store.errors.is_empty());
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_refused_reopen_is_reported_on_its_pr_without_a_refetch() {
    let gh = FakeGh::new()
        .fail(
            "pulls/1 -f state=open",
            1,
            "gh: Validation Failed: the head branch was deleted (HTTP 422)",
        )
        .install();
    let mut app = app();

    reopen(&mut app, 1);
    settle(&mut app).await;

    assert_eq!(gh.calls().len(), 1, "no refetch after a failed write");
    assert!(
        app.state.store.errors[&1].contains("head branch was deleted"),
        "{:?}",
        app.state.store.errors
    );
    assert!(app.state.store.operations.is_empty());
}
