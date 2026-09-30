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
    action::{Action, Command},
    reviews::CommentTarget,
    store::{FetchKey, LoadState},
};
use crate::{
    providers::Provider,
    test_support::{FakeGh, gh_list_page, gh_pr},
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
    let gh = FakeGh::new().on("pullRequests(", &one_pr_page()).install();
    let mut app = app();

    app.spawn_load_prs();
    app.spawn_load_prs();
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1]);
    assert_eq!(gh.calls().len(), 1, "{:?}", gh.calls());
    assert!(app.state.store.refresh_failures.is_empty());
}

#[tokio::test]
async fn failed_refresh_keeps_the_list_then_recovers() {
    let gh = FakeGh::new()
        .once("pullRequests(", &one_pr_page())
        .fail_once("pullRequests(", 1, "gh: HTTP 502: Bad Gateway")
        .on("pullRequests(", &one_pr_page())
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
    assert_eq!(gh.calls().len(), 3);
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
        .on("pullRequests(", &one_pr_page())
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
        ("the PR list", "pullRequests(first: 100"),
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
        .on("pullRequests(", &one_pr_page())
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
