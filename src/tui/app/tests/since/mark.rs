//! What counts as having read a diff, and so when the mark shows and goes.

use super::support::*;

#[test]
fn the_head_of_the_diff_that_was_opened_is_kept_and_the_overview_keeps_nothing() {
    let mut app = app();
    branch_at(&mut app, "aaa111");
    diff_of(&mut app, "aaa111");
    detail(&mut app, DetailTab::Overview);
    assert_eq!(read_head(&app), None, "not opened yet");
    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(
        DetailTab::Diff,
    ))));
    app.apply_result(TaskResult::Read(Read::Diff(
        PrId(42),
        Ok(Diff {
            revision: Some(DiffRevision {
                head: "aaa111".into(),
                base: None,
                commit: false,
            }),
            files: vec![],
        }),
    )));
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
}

#[tokio::test(flavor = "current_thread")]
async fn a_branch_that_moved_says_so_and_w_shows_what_is_new() {
    let mut app = returning_reader("aaa111", "bbb222");
    assert!(screen_text(&mut app).contains(MARK), "the header says so");

    press(&mut app, KeyCode::Char('w'));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Diff
        },
        "w goes to the Diff tab"
    );
    assert!(app.state.ui.detail.since.is_some());
    // The whole diff here is of the older head. The PR's diff of the new one is
    // read first: it says which files are the PR's.
    let wanted = FetchKey::Pr(PrResource::RangeDiff(range("aaa111", "bbb222")), PrId(42));
    assert!(app.state.store.fetches.contains(&WHOLE));
    assert!(!app.state.store.fetches.contains(&wanted));
    diff_arrives(&mut app, "bbb222", &[]);
    assert!(app.state.store.fetches.contains(&wanted));

    // Until it is read, the head read is still the old one.
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Ok(Diff {
            revision: None,
            files: vec![],
        }),
    )));
    assert_eq!(
        read_head(&app),
        Some(&oid("bbb222")),
        "what is new was read"
    );

    // `w` again is the whole diff, and the mark is gone for good.
    press(&mut app, KeyCode::Char('w'));
    assert!(app.state.ui.detail.since.is_none());
    assert!(!screen_text(&mut app).contains(MARK));
}

#[test]
fn nothing_is_offered_while_the_branch_is_where_the_reader_left_it() {
    let mut app = returning_reader("aaa111", "aaa111");
    assert!(!screen_text(&mut app).contains(MARK));
    press(&mut app, KeyCode::Char('w'));
    assert!(app.state.ui.detail.since.is_none());
    assert!(app.state.store.fetches.is_empty());
}

#[test]
fn a_pr_whose_diff_was_never_opened_has_nothing_to_be_new_since() {
    let mut app = app();
    branch_at(&mut app, "bbb222");
    detail(&mut app, DetailTab::Overview);
    assert!(!screen_text(&mut app).contains(MARK));
}

#[test]
fn a_provider_that_cannot_compare_has_no_mark_and_no_key() {
    let mut app = returning_reader("aaa111", "bbb222");
    app.state
        .store
        .capabilities
        .features
        .remove(&crate::domain::capabilities::Feature::RangeDiff);
    assert!(!screen_text(&mut app).contains(MARK));
    press(&mut app, KeyCode::Char('w'));
    assert!(app.state.ui.detail.since.is_none());
}

#[test]
fn a_diff_that_is_not_the_branch_any_more_is_not_what_was_read() {
    let mut app = returning_reader("aaa111", "bbb222");
    // The Diff tab shows the old diff for a while after the branch moved.
    detail(&mut app, DetailTab::Diff);
    assert_eq!(read_head(&app), Some(&oid("aaa111")), "kept as it was");
    assert!(screen_text(&mut app).contains(MARK), "still says it is new");
}

#[tokio::test(flavor = "current_thread")]
async fn a_refresh_that_swaps_in_the_newer_diff_does_not_read_it_for_the_reader() {
    let mut app = returning_reader("aaa111", "bbb222");
    detail(&mut app, DetailTab::Diff);
    assert_eq!(
        read_head(&app),
        Some(&oid("aaa111")),
        "the old diff is on screen"
    );

    // The minute passes: the diff is read again, and is of the branch as it is.
    app.apply_result(TaskResult::Read(Read::Diff(
        PrId(42),
        Ok(Diff {
            revision: Some(DiffRevision {
                head: "bbb222".into(),
                base: Some("0ba5e0".into()),
                commit: false,
            }),
            files: vec![],
        }),
    )));
    assert_eq!(read_head(&app), Some(&oid("aaa111")), "nobody read it");
    assert!(screen_text(&mut app).contains(MARK), "so the mark stays");
    // Nor does a key pressed on the diff clear it.
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    assert!(screen_text(&mut app).contains(MARK));
    // Nor the next minute's refresh, which brings the same diff again, nor `F`.
    diff_arrives(&mut app, "bbb222", &[]);
    press(&mut app, KeyCode::Char('F'));
    diff_arrives(&mut app, "bbb222", &[]);
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    assert!(screen_text(&mut app).contains(MARK));
}

#[test]
fn the_diff_of_another_pr_that_arrives_does_not_read_the_one_on_screen() {
    let mut app = returning_reader("aaa111", "bbb222");
    detail(&mut app, DetailTab::Diff);
    diff_arrives(&mut app, "bbb222", &[]);
    app.apply_result(TaskResult::Read(Read::Diff(
        PrId(7),
        Ok(Diff {
            revision: Some(DiffRevision {
                head: "ddd444".into(),
                base: None,
                commit: false,
            }),
            files: vec![],
        }),
    )));
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    assert!(screen_text(&mut app).contains(MARK));
}

#[tokio::test(flavor = "current_thread")]
async fn what_is_new_that_was_read_after_the_reader_left_is_read_when_it_is_shown() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    // Gone before the compare answers.
    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(
        DetailTab::Overview,
    ))));
    compare_arrives(&mut app, &["src/main.rs"]);
    assert_eq!(read_head(&app), Some(&oid("aaa111")), "nobody saw it");
    assert!(screen_text(&mut app).contains(MARK));

    press(&mut app, KeyCode::Char('w'));
    assert_eq!(read_head(&app), Some(&oid("bbb222")), "now it is on screen");
}
