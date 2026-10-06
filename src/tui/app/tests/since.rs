//! A PR that has moved since the reader had its diff open says so, and `w`
//! shows what is new.

use super::support::*;
use crate::{
    domain::{
        commit::CommitOid,
        diff::{Diff, DiffRange, DiffRevision},
    },
    tui::app::store::{FetchKey, PrResource},
};
use ratatui::{Terminal, backend::TestBackend};

const MARK: &str = "new since you read it";

fn oid(text: &str) -> CommitOid {
    CommitOid::parse(text).expect("a commit id")
}

fn range(base: &str, head: &str) -> DiffRange {
    DiffRange {
        base: oid(base),
        head: oid(head),
    }
}

/// The diff the PR screen holds was made from this head.
fn diff_of(app: &mut App, head: &str) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.diff = LoadState::Loaded(Diff {
            revision: Some(DiffRevision {
                head: head.into(),
                base: Some("0ba5e0".into()),
                commit: false,
            }),
            files: vec![],
        });
    }
}

/// The list says the branch is at `head` now.
fn branch_at(app: &mut App, head: &str) {
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        for pr in prs.iter_mut().filter(|pr| pr.id == PrId(42)) {
            pr.head_oid = Some(head.into());
        }
    }
}

fn screen_text(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &mut app.state))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn read_head(app: &App) -> Option<&CommitOid> {
    app.state.store.seen.read_head(PrId(42))
}

/// A reader who had `read` open last time, and finds the branch at `now`.
fn returning_reader(read: &str, now: &str) -> App {
    let mut app = app();
    branch_at(&mut app, read);
    diff_of(&mut app, read);
    detail(&mut app, DetailTab::Diff);
    assert_eq!(read_head(&app), Some(&oid(read)), "the diff was opened");
    press(&mut app, KeyCode::Esc);
    branch_at(&mut app, now);
    detail(&mut app, DetailTab::Overview);
    app
}

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
    let wanted = FetchKey::Pr(PrResource::RangeDiff(range("aaa111", "bbb222")), PrId(42));
    assert!(app.state.store.fetches.contains(&wanted));
    assert!(app.state.ui.detail.since.is_some());

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

#[tokio::test(flavor = "current_thread")]
async fn no_comment_is_made_on_what_is_new_and_leaving_the_tab_closes_it() {
    let mut app = returning_reader("aaa111", "bbb222");
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Char('c'));
    assert!(!app.state.ui.detail.modal_open(), "no editor on its lines");

    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(
        DetailTab::Overview,
    ))));
    assert!(app.state.ui.detail.since.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn esc_in_what_is_new_goes_back_to_the_whole_diff_not_to_the_list() {
    let mut app = returning_reader("aaa111", "bbb222");
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.since.is_none());
    assert!(matches!(
        app.state.screen,
        Screen::Detail {
            tab: DetailTab::Diff,
            ..
        }
    ));
}

#[test]
fn a_diff_that_is_not_the_branch_any_more_is_not_what_was_read() {
    let mut app = returning_reader("aaa111", "bbb222");
    // The Diff tab shows the old diff for a while after the branch moved.
    detail(&mut app, DetailTab::Diff);
    assert_eq!(read_head(&app), Some(&oid("aaa111")), "kept as it was");
    assert!(screen_text(&mut app).contains(MARK), "still says it is new");
}
