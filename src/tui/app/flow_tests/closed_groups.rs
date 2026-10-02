//! Merged and declined PRs: read per view, and older ones on request.

use super::support::*;

fn empty_page() -> String {
    gh_list_page(&[], None)
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
    app.apply(Action::Effect(Effect::LoadView));
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
    assert_eq!(notice.kind, crate::tui::app::store::NoticeKind::Error);
    assert!(notice.message.contains("Couldn't load older PRs"));
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
    assert_eq!(
        notice.kind,
        crate::tui::app::store::NoticeKind::Error,
        "{notice:?}"
    );

    switch_to(&mut app, StatusFilter::Open);
    switch_to(&mut app, StatusFilter::Merged);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1, 3]);
    assert_eq!(list_reads(&gh.calls()), (1, 2, 0), "{:?}", gh.calls());
}
