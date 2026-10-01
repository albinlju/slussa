//! The open group: first load, refresh, and reading it a page at a time.

use super::support::*;

/// Take exactly one provider result and apply it.
async fn apply_next(app: &mut App) {
    let action = tokio::time::timeout(Duration::from_secs(10), app.action_rx.recv())
        .await
        .expect("a provider result within 10 seconds")
        .expect("the action channel stays open");
    app.apply(action);
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
