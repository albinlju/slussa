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
    store::{FetchKey, LoadState, OpenChain},
};
use crate::{
    domain::pr::PrGroup,
    providers::Provider,
    test_support::{FakeGh, gh_closed_pr, gh_list_page, gh_pr},
    tui::screens::pr_list::StatusFilter,
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

/// Take exactly one provider result and apply it.
async fn apply_next(app: &mut App) {
    let action = tokio::time::timeout(Duration::from_secs(10), app.action_rx.recv())
        .await
        .expect("a provider result within 10 seconds")
        .expect("the action channel stays open");
    app.apply(action);
}

fn one_pr_page() -> String {
    gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], None)
}

/// Each group of PRs is a read of its own: the open ones, the merged ones and
/// the declined ones. A view reads only the groups it shows.
const OPEN_QUERY: &str = "states: OPEN";
const MERGED_QUERY: &str = "states: MERGED";
const DECLINED_QUERY: &str = "states: CLOSED";

fn empty_page() -> String {
    gh_list_page(&[], None)
}

/// How many reads of the open, merged and declined groups were made.
fn list_reads(calls: &[String]) -> (usize, usize, usize) {
    let count = |needle: &str| calls.iter().filter(|call| call.contains(needle)).count();
    (
        count(OPEN_QUERY),
        count(MERGED_QUERY),
        count(DECLINED_QUERY),
    )
}

/// An answer shaped like every connection the refetch touches, so each read
/// gets something parseable without modelling GitHub.
fn any_connection() -> String {
    let connection = json!({"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}});
    json!({"data": {
        "repository": {
            "connection": connection,
            "item": {"connection": connection},
            "pullRequest": {"mergeable": "MERGEABLE", "id": "PR_1", "body": null, "labels": connection}
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
async fn the_open_group_loads_through_the_provider_and_a_repeat_request_reuses_it() {
    let gh = FakeGh::new().on(OPEN_QUERY, &one_pr_page()).install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1]);
    assert_eq!(
        list_reads(&gh.calls()),
        (1, 0, 0),
        "only what the view shows is read, once: {:?}",
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
        .install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1]);

    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1], "stale data stays visible");
    assert!(
        app.state
            .store
            .refresh_failures
            .contains(&FetchKey::Prs(PrGroup::Open)),
        "the failure is recorded so the UI can say so"
    );

    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert!(app.state.store.refresh_failures.is_empty());
    assert_eq!(list_reads(&gh.calls()), (3, 0, 0), "{:?}", gh.calls());
}

#[tokio::test]
async fn first_load_failure_is_shown_as_failed_not_empty() {
    let _gh = FakeGh::new()
        .fail("pullRequests(", 1, "gh: HTTP 401: Bad credentials")
        .install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
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

fn merged(id: u64, at: &str) -> serde_json::Value {
    gh_closed_pr(id, at, "MERGED")
}

/// A fake whose merged PRs span three reads: the first page (with a cursor),
/// then `after: "x"` (with another), then `after: "y"` (the end). The cursor
/// rules come first because the first page's rule matches them too.
fn history() -> FakeGh {
    FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .on(
            "after: \"x\"",
            &gh_list_page(&[merged(2, "2026-07-01T10:00:00Z")], Some("y")),
        )
        .on(
            "after: \"y\"",
            &gh_list_page(&[merged(4, "2026-06-01T10:00:00Z")], None),
        )
        .on(
            MERGED_QUERY,
            &gh_list_page(&[merged(3, "2026-08-01T10:00:00Z")], Some("x")),
        )
        .on(DECLINED_QUERY, &empty_page())
}

fn older_reads(calls: &[String]) -> usize {
    calls
        .iter()
        .filter(|call| call.contains("after: \"x\"") || call.contains("after: \"y\""))
        .count()
}

fn merged_cursor(app: &App) -> Option<String> {
    app.state
        .store
        .groups
        .get(&PrGroup::Merged)
        .and_then(|state| state.more.clone())
}

fn switch_to(app: &mut App, filter: StatusFilter) {
    app.state.ui.list.filter = filter;
    app.apply(Action::List(ListAction::FilterChanged));
}

/// Start the app the way `run` does, then look at the merged PRs.
async fn load_merged_first_page(app: &mut App) {
    app.spawn_load_prs(PrGroup::Open, None);
    settle(app).await;
    switch_to(app, StatusFilter::Merged);
    settle(app).await;
}

#[tokio::test]
async fn the_start_reads_only_the_open_group() {
    let gh = history().install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1]);
    assert_eq!(list_reads(&gh.calls()), (1, 0, 0), "{:?}", gh.calls());
}

#[tokio::test]
async fn switching_to_a_view_reads_its_group_once() {
    let gh = history().install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;

    switch_to(&mut app, StatusFilter::Merged);
    switch_to(&mut app, StatusFilter::Merged);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3]);
    assert_eq!(list_reads(&gh.calls()), (1, 1, 0), "{:?}", gh.calls());

    switch_to(&mut app, StatusFilter::Open);
    switch_to(&mut app, StatusFilter::Merged);
    settle(&mut app).await;
    assert_eq!(
        list_reads(&gh.calls()),
        (1, 1, 0),
        "a group already read is not read again: {:?}",
        gh.calls()
    );
}

#[tokio::test]
async fn the_all_view_reads_the_groups_still_missing() {
    let gh = history().install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    switch_to(&mut app, StatusFilter::Merged);
    settle(&mut app).await;

    switch_to(&mut app, StatusFilter::All);
    settle(&mut app).await;

    assert_eq!(list_reads(&gh.calls()), (1, 1, 1), "{:?}", gh.calls());
    assert_eq!(loaded_ids(&app), vec![1, 3]);
}

#[tokio::test]
async fn a_refresh_reads_the_open_group_and_the_groups_already_read() {
    let gh = history().install();
    let mut app = app();
    load_merged_first_page(&mut app).await;
    assert_eq!(list_reads(&gh.calls()), (1, 1, 0));

    app.refresh_list();
    settle(&mut app).await;

    assert_eq!(
        list_reads(&gh.calls()),
        (2, 2, 0),
        "the declined group was never opened: {:?}",
        gh.calls()
    );
}

#[tokio::test]
async fn older_prs_are_appended_until_the_history_ends() {
    let gh = history().install();
    let mut app = app();
    load_merged_first_page(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3]);
    assert_eq!(merged_cursor(&app).as_deref(), Some("x"));

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2]);
    assert_eq!(merged_cursor(&app).as_deref(), Some("y"));
    assert_eq!(
        app.state.store.notice.as_ref().map(|n| n.message.as_str()),
        Some("Loaded 1 older PR")
    );

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2, 4]);
    assert_eq!(merged_cursor(&app), None, "the history has ended");

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
    load_merged_first_page(&mut app).await;

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
    load_merged_first_page(&mut app).await;
    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3, 2]);

    // The minute-by-minute refresh reads the first pages again.
    app.refresh_list();
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1, 3, 2], "PR 2 is not thrown away");
    assert_eq!(
        merged_cursor(&app).as_deref(),
        Some("y"),
        "and the place reached is kept, not reset to the first page's"
    );
}

#[tokio::test]
async fn a_failed_older_read_keeps_the_list_says_so_and_can_be_retried() {
    let gh = FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .fail_once("after: \"x\"", 1, "gh: HTTP 502: Bad Gateway")
        .on(
            "after: \"x\"",
            &gh_list_page(&[merged(2, "2026-07-01T10:00:00Z")], None),
        )
        .on(
            MERGED_QUERY,
            &gh_list_page(&[merged(3, "2026-08-01T10:00:00Z")], Some("x")),
        )
        .install();
    let mut app = app();
    load_merged_first_page(&mut app).await;

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3], "the list is untouched");
    assert_eq!(merged_cursor(&app).as_deref(), Some("x"));
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

#[tokio::test]
async fn a_failed_first_read_of_a_group_is_reported_and_retried_on_the_next_visit() {
    let gh = FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .fail_once(MERGED_QUERY, 1, "gh: HTTP 502: Bad Gateway")
        .on(
            MERGED_QUERY,
            &gh_list_page(&[merged(3, "2026-08-01T10:00:00Z")], None),
        )
        .install();
    let mut app = app();
    load_merged_first_page(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1], "the open PRs are untouched");
    let notice = app
        .state
        .store
        .notice
        .as_ref()
        .expect("the failure is said");
    assert!(notice.error, "{notice:?}");

    switch_to(&mut app, StatusFilter::Open);
    switch_to(&mut app, StatusFilter::Merged);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3]);
    assert_eq!(list_reads(&gh.calls()), (1, 2, 0), "{:?}", gh.calls());
}

// ---------------------------------------------------------------------------
// The open group is read a page at a time
// ---------------------------------------------------------------------------

fn open_page(ids: &[u64], more: Option<&str>) -> String {
    let prs: Vec<_> = ids
        .iter()
        .map(|&id| gh_pr(id, &format!("2026-09-0{id}T10:00:00Z")))
        .collect();
    gh_list_page(&prs, more)
}

/// Three open pages: PRs 3 and 2, then 1, then 0.
fn three_open_pages() -> FakeGh {
    FakeGh::new()
        .on("after: \"p2\"", &open_page(&[4], None))
        .on("after: \"p1\"", &open_page(&[2], Some("p2")))
        .on(OPEN_QUERY, &open_page(&[1], Some("p1")))
}

#[tokio::test]
async fn the_first_reading_shows_each_open_page_as_it_arrives() {
    let gh = three_open_pages().install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    apply_next(&mut app).await;
    assert_eq!(
        loaded_ids(&app),
        vec![1],
        "the list appears with the first page"
    );
    assert!(matches!(app.state.store.open_chain, OpenChain::Appending));
    assert!(
        app.state
            .store
            .fetches
            .contains(&FetchKey::Prs(PrGroup::Open)),
        "and the next page is already being read"
    );

    settle(&mut app).await;
    let mut ids = loaded_ids(&app);
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 4]);
    assert!(matches!(app.state.store.open_chain, OpenChain::Idle));
    assert!(
        !app.state
            .store
            .fetches
            .contains(&FetchKey::Prs(PrGroup::Open))
    );
    assert_eq!(list_reads(&gh.calls()), (3, 0, 0), "{:?}", gh.calls());
}

#[tokio::test]
async fn a_refresh_keeps_the_whole_list_until_its_last_page_arrives() {
    let _gh = three_open_pages().install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app).len(), 3);

    app.spawn_load_prs(PrGroup::Open, None);
    apply_next(&mut app).await;
    assert!(matches!(
        app.state.store.open_chain,
        OpenChain::Collecting(_)
    ));
    assert_eq!(
        loaded_ids(&app).len(),
        3,
        "the list does not shrink to the first page while the rest is read"
    );

    settle(&mut app).await;
    assert_eq!(loaded_ids(&app).len(), 3);
    assert!(matches!(app.state.store.open_chain, OpenChain::Idle));
}

#[tokio::test]
async fn a_failed_page_keeps_what_is_shown_and_says_the_list_is_stale() {
    let _gh = FakeGh::new()
        .fail("after: \"p1\"", 1, "gh: HTTP 502: Bad Gateway")
        .on(OPEN_QUERY, &open_page(&[1], Some("p1")))
        .install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app), vec![1]);
    assert!(matches!(app.state.store.open_chain, OpenChain::Idle));
    assert!(
        app.state
            .store
            .refresh_failures
            .contains(&FetchKey::Prs(PrGroup::Open))
    );
}

/// A page of 30 open PRs numbered from `first`, newest first.
fn full_page(first: u64, more: Option<&str>) -> String {
    let prs: Vec<_> = (first..first + 30)
        .rev()
        .map(|id| gh_pr(id, &format!("2026-01-01T{:02}:{:02}:00Z", id / 60, id % 60)))
        .collect();
    gh_list_page(&prs, more)
}

/// Four pages of 30: the limit is three.
fn four_full_pages() -> FakeGh {
    FakeGh::new()
        .on("after: \"p3\"", &full_page(91, None))
        .on("after: \"p2\"", &full_page(61, Some("p3")))
        .on("after: \"p1\"", &full_page(31, Some("p2")))
        .on(OPEN_QUERY, &full_page(1, Some("p1")))
}

fn open_cursor(app: &App) -> Option<String> {
    app.state
        .store
        .groups
        .get(&PrGroup::Open)
        .and_then(|state| state.more.clone())
}

#[tokio::test]
async fn the_open_group_stops_at_the_limit_and_l_reads_the_next_batch() {
    let gh = four_full_pages().install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    apply_next(&mut app).await;
    assert!(
        app.state.ui.list.hold_order,
        "the order is held while reading"
    );
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app).len(), 90, "three pages, then it stops");
    assert_eq!(list_reads(&gh.calls()), (3, 0, 0), "{:?}", gh.calls());
    assert_eq!(open_cursor(&app).as_deref(), Some("p3"), "L continues here");
    assert!(
        !app.state.ui.list.hold_order,
        "the order applies once it ends"
    );
    assert!(matches!(app.state.store.open_chain, OpenChain::Idle));

    app.apply(Action::List(ListAction::LoadOlder));
    assert!(app.state.ui.list.hold_order);
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app).len(), 120);
    assert_eq!(open_cursor(&app), None, "nothing is left");
    assert!(!app.state.ui.list.hold_order);
    assert_eq!(list_reads(&gh.calls()), (4, 0, 0), "{:?}", gh.calls());
}

#[tokio::test]
async fn pressing_l_twice_while_the_open_group_is_read_makes_one_request() {
    let gh = four_full_pages().install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;

    app.apply(Action::List(ListAction::LoadOlder));
    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;

    assert_eq!(list_reads(&gh.calls()), (4, 0, 0), "{:?}", gh.calls());
    assert_eq!(loaded_ids(&app).len(), 120);
}

#[tokio::test]
async fn a_refresh_reads_as_far_as_the_list_already_reaches() {
    let gh = four_full_pages().install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app).len(), 120);

    app.refresh_list();
    settle(&mut app).await;

    assert_eq!(loaded_ids(&app).len(), 120, "what L added is kept");
    assert_eq!(list_reads(&gh.calls()), (8, 0, 0), "{:?}", gh.calls());
}

#[tokio::test]
async fn a_failed_continuation_keeps_the_list_and_l_retries_from_there() {
    let gh = FakeGh::new()
        .fail_once("after: \"p1\"", 1, "gh: HTTP 502: Bad Gateway")
        .on("after: \"p1\"", &full_page(31, None))
        .on(OPEN_QUERY, &full_page(1, Some("p1")))
        .install();
    let mut app = app();

    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app).len(), 30);
    assert!(!app.state.ui.list.hold_order);
    assert_eq!(open_cursor(&app).as_deref(), Some("p1"));

    app.apply(Action::List(ListAction::LoadOlder));
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app).len(), 60);
    assert_eq!(open_cursor(&app), None);
    assert_eq!(list_reads(&gh.calls()), (3, 0, 0), "{:?}", gh.calls());
}

// ---------------------------------------------------------------------------
// The description and labels are read when a PR is opened
// ---------------------------------------------------------------------------

fn info_reads(calls: &[String]) -> usize {
    calls
        .iter()
        .filter(|call| call.contains("pullRequest(number: $pr) { id body"))
        .count()
}

fn info_answer() -> String {
    json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Why this change.",
        "labels": {"nodes": [{"name": "bug"}], "pageInfo": {"hasNextPage": false}}
    }}}})
    .to_string()
}

#[tokio::test]
async fn opening_a_pr_reads_its_description_and_labels_once() {
    let gh = FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .on("pullRequest(number: $pr) { id body", &info_answer())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1]);
    assert_eq!(info_reads(&gh.calls()), 0, "the list does not read them");

    app.apply(Action::List(ListAction::OpenPr(1)));
    app.apply(Action::List(ListAction::OpenPr(1)));
    settle(&mut app).await;

    let info = match &app.state.store.cache.details[&1].info {
        LoadState::Loaded(info) => info.clone(),
        other => panic!("expected the info to be loaded, got {other:?}"),
    };
    assert_eq!(info.description.as_deref(), Some("Why this change."));
    assert_eq!(info.labels, vec!["bug"]);
    assert_eq!(info_reads(&gh.calls()), 1, "{:?}", gh.calls());
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
