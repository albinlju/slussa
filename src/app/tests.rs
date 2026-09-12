use super::{App, action::*};
use crate::{
    app::{
        navigation::Screen,
        reviews::{CommentAnchor, CommentTarget},
        store::LoadState,
    },
    providers::Provider,
    tui::{
        self,
        component::Component,
        components::diff_viewer::DiffFocus,
        screens::pr_detail::{
            dialogs::{
                confirm::{ConfirmDialog, ConfirmKind},
                review::ReviewDialog,
            },
            tabs::DetailTab,
        },
    },
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn app() -> App {
    let mut app = App::new(Provider::GitHub, "reviewer".into());
    app.state = tui::regression_tests::fixture();
    app
}

fn press(app: &mut App, code: KeyCode) {
    if let Some(action) = tui::key_to_action(&app.state, KeyEvent::new(code, KeyModifiers::NONE)) {
        app.apply(action);
    }
}

fn detail(app: &mut App, tab: DetailTab) {
    app.apply(Action::List(ListAction::OpenPr(42)));
    app.apply(Action::Detail(DetailAction::SelectTab(tab)));
}

#[test]
fn navigation_and_search_keep_the_same_keyboard_flow() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    for c in "missing".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert!(
        app.state
            .ui
            .list
            .filtered_prs(&app.state.store.cache.prs)
            .is_empty()
    );
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Description
        }
    );
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.screen, Screen::List);
}

#[test]
fn filter_picker_applies_and_cancels_without_leaking_keys() {
    let mut app = app();
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        app.state.ui.list.filter,
        tui::screens::pr_list::StatusFilter::Open
    );
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.ui.list.filter,
        tui::screens::pr_list::StatusFilter::Draft
    );
    assert_eq!(app.state.ui.list.selected, 0);
}

#[test]
fn diff_search_focus_and_match_wrapping_are_local() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    press(&mut app, KeyCode::Char('/'));
    for c in "main".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.state.ui.detail.diff.focused_file, 0);
    press(&mut app, KeyCode::Esc);
    // root directory first; move to the file before entering the pane
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Pane);
    app.state.ui.detail.diff.pane_matches = vec![2, 5];
    app.state.ui.detail.diff.pane_search.query = "new".into();
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 2);
    press(&mut app, KeyCode::Char('N'));
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 5);
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.diff.pane_search.query.is_empty());
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Pane);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Tree);
}

#[test]
fn commit_drilldown_uses_an_independent_diff_instance() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.state.ui.detail.diff.pane_cursor = 7;
    let data = app.state.store.cache.details.get_mut(&42).unwrap();
    let LoadState::Loaded(diff) = &data.diff else {
        panic!()
    };
    data.commit_diffs
        .insert("abcdef123456".into(), LoadState::Loaded(diff.clone()));
    app.apply(Action::Detail(DetailAction::SelectTab(DetailTab::Commits)));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.ui.detail.commits.open_commit.as_deref(),
        Some("abcdef123456")
    );
    app.state.ui.detail.commits.diff.pane_item_count = 5;
    app.apply(Action::Diff(DiffAction::MovePaneCursor(2)));
    assert_eq!(app.state.ui.detail.commits.diff.pane_cursor, 2);
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 7);
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.commits.open_commit.is_none());
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 7);
}

#[test]
fn commit_component_emits_a_pr_scoped_load_request() {
    let mut app = app();
    let ctx = tui::screens::pr_detail::tabs::commits::CommitContext {
        pr_id: 42,
        data: app.state.store.cache.details.get(&42),
        pending: &[],
        author: "alice",
    };
    let effect = app
        .state
        .ui
        .detail
        .commits
        .update(CommitsAction::Open, &ctx);
    assert!(
        matches!(effect, Some(Action::LoadCommitDiff { pr_id: 42, oid }) if oid == "abcdef123456")
    );
    assert!(
        app.state
            .ui
            .detail
            .commits
            .update(CommitsAction::StepCommit(1), &ctx)
            .is_none()
    );
}

#[test]
fn queued_review_survives_navigation_without_crossing_prs() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    press(&mut app, KeyCode::Char('v'));
    app.state.ui.detail.diff.focus = DiffFocus::Pane;
    app.state.ui.detail.diff.pane_anchor = Some(CommentAnchor {
        path: "src/main.rs".into(),
        line: 1,
        removed: false,
    });
    press(&mut app, KeyCode::Char('c'));
    for c in "Please explain".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.store.reviews[&42].comments[0].text,
        "Please explain"
    );
    // Opening another PR uses already cached data and never contacts a provider.
    let data = app.state.store.cache.details.remove(&42).unwrap();
    app.state.store.cache.details.insert(43, data);
    app.apply(Action::List(ListAction::OpenPr(43)));
    assert!(app.state.detail_view().pending_review().is_none());
    app.apply(Action::Detail(DetailAction::StartReview));
    assert!(app.state.store.reviews[&43].comments.is_empty());
    app.apply(Action::Detail(DetailAction::AbandonReview));
    assert_eq!(app.state.store.reviews[&42].comments.len(), 1);
}

#[test]
fn editor_captures_shortcuts_and_unicode_backspace() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    for c in "qå🦀".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Backspace);
    assert_eq!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "qå"
    );
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.editor.draft.is_none());
}

#[test]
fn own_pr_review_gate_and_decline_cancel_are_preserved() {
    let mut app = app();
    app.state.store.current_user = "alice".into();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(
        app.state
            .ui
            .detail
            .review_picker
            .as_ref()
            .map(ReviewDialog::cursor),
        Some(2)
    );
    press(&mut app, KeyCode::Enter);
    assert!(matches!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().target,
        CommentTarget::Review {
            verdict: crate::domain::review::ReviewVerdict::Comment
        }
    ));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('x'));
    assert_eq!(
        app.state
            .ui
            .detail
            .confirm
            .as_ref()
            .map(ConfirmDialog::kind),
        Some(ConfirmKind::Decline)
    );
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Enter);
    assert!(app.state.ui.detail.confirm.is_none());
    assert!(!app.state.store.operations.contains_key(&42));
}

#[test]
fn refresh_failure_preserves_visible_data() {
    let mut app = app();
    app.apply(Action::Loaded(LoadedAction::Prs(Err("offline".into()))));
    assert!(matches!(&app.state.store.cache.prs, LoadState::Loaded(prs) if prs.len() == 1));
}

#[test]
fn review_picker_captures_keys_before_diff_search() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    press(&mut app, KeyCode::Char('v'));
    press(&mut app, KeyCode::Char('v'));
    assert!(app.state.ui.detail.review_picker.is_some());
    press(&mut app, KeyCode::Char('/'));
    assert!(!app.state.ui.detail.diff.tree_search.open);
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.review_picker.is_none());
    assert!(app.state.store.reviews.contains_key(&42));
}

#[test]
fn overview_navigation_resets_reply_selection() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state.ui.detail.overview.timeline.item_count = 3;
    app.state.ui.detail.overview.timeline.sub = 2;
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.state.ui.detail.overview.timeline.cursor, 1);
    assert_eq!(app.state.ui.detail.overview.timeline.sub, 0);
    app.state.ui.detail.overview.timeline.item_count = 1;
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.state.ui.detail.overview.timeline.scroll, 1);
}

// These current-thread tests deliberately never yield: provider futures are
// queued but never polled. We inject their results through the real dispatcher;
// dropping the runtime cancels the queued work without contacting a provider.
#[tokio::test(flavor = "current_thread")]
async fn failed_submission_preserves_draft_and_blocks_duplicate_input() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    for c in "Keep this draft".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(app.state.store.operations.contains_key(&42));
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.store.operations.len(), 1);
    assert_eq!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "Keep this draft"
    );
    // A read result must never acknowledge a write.
    app.apply(Action::Loaded(LoadedAction::Activity(
        42,
        Err("read failed".into()),
    )));
    assert!(app.state.store.operations.contains_key(&42));
    app.apply(Action::Loaded(LoadedAction::Commented(
        42,
        Err("offline".into()),
    )));
    assert!(!app.state.store.operations.contains_key(&42));
    assert_eq!(
        app.state.store.errors.get(&42).map(String::as_str),
        Some("offline")
    );
    press(&mut app, KeyCode::Enter); // dismiss error, do not resubmit
    assert!(!app.state.store.operations.contains_key(&42));
    assert!(app.state.store.errors.is_empty());
    assert_eq!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "Keep this draft"
    );
    press(&mut app, KeyCode::Enter);
    app.apply(Action::Loaded(LoadedAction::Commented(42, Ok(()))));
    assert!(app.state.ui.detail.editor.draft.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn late_completion_only_clears_the_submitting_pr_editor() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        let mut second = prs[0].clone();
        second.id = 43;
        prs.push(second);
    }
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Enter);
    app.open_pr(43);
    app.apply(Action::Detail(DetailAction::SelectTab(DetailTab::Overview)));
    app.apply(Action::Detail(DetailAction::OpenComment));
    app.apply(Action::Detail(DetailAction::CommentType('B')));
    app.apply(Action::Loaded(LoadedAction::Commented(42, Ok(()))));
    assert_eq!(app.state.ui.detail.editor.draft.as_ref().unwrap().text, "B");
    app.open_pr(42);
    assert!(app.state.ui.detail.editor.draft.is_none());
    app.open_pr(43);
    assert_eq!(app.state.ui.detail.editor.draft.as_ref().unwrap().text, "B");
}

#[tokio::test(flavor = "current_thread")]
async fn failed_review_and_error_stay_with_their_pr_until_success() {
    use crate::{
        app::{
            reviews::{PendingComment, PendingReview},
            store::Operation,
        },
        domain::review::ReviewVerdict,
    };
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state.store.reviews.insert(
        42,
        PendingReview {
            comments: vec![PendingComment {
                anchor: CommentAnchor {
                    path: "file.rs".into(),
                    line: 1,
                    removed: false,
                },
                text: "review comment".into(),
            }],
        },
    );
    app.apply(Action::Command {
        pr_id: 42,
        command: Command::SubmitReview {
            verdict: ReviewVerdict::Approve,
            body: String::new(),
        },
    });
    assert_eq!(app.state.store.reviews[&42].comments.len(), 1);
    app.open_pr(43);
    app.state.store.operations.insert(43, Operation::Merge);
    app.apply(Action::Loaded(LoadedAction::Commented(
        42,
        Err("offline".into()),
    )));
    assert!(app.state.store.operations.contains_key(&43));
    assert!(app.state.detail_view().error().is_none());
    assert_eq!(app.state.store.reviews[&42].comments.len(), 1);
    app.open_pr(42);
    assert_eq!(app.state.detail_view().error(), Some("offline"));
    press(&mut app, KeyCode::Esc);
    app.apply(Action::Command {
        pr_id: 42,
        command: Command::SubmitReview {
            verdict: ReviewVerdict::Approve,
            body: String::new(),
        },
    });
    app.apply(Action::Loaded(LoadedAction::Commented(42, Ok(()))));
    assert!(!app.state.store.reviews.contains_key(&42));
    assert!(app.state.store.operations.contains_key(&43));
}

#[tokio::test(flavor = "current_thread")]
async fn refresh_tracks_each_resource_and_refetches_after_mutation() {
    use crate::app::store::{FetchKey, Operation};
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.apply(Action::Refresh);
    app.apply(Action::Refresh);
    assert_eq!(app.state.store.fetches.len(), 3);
    assert!(app.state.store.refreshing(app.state.screen));
    app.apply(Action::Loaded(LoadedAction::Prs(Err("offline".into()))));
    assert!(app.state.store.refreshing(app.state.screen));
    app.state.store.operations.insert(42, Operation::Moderation);
    app.apply(Action::Loaded(LoadedAction::Commented(42, Ok(()))));
    assert!(
        app.state
            .store
            .reload_after_fetch
            .contains(&FetchKey::Activity(42))
    );
    app.apply(Action::Loaded(LoadedAction::Activity(
        42,
        Err("old response".into()),
    )));
    assert!(app.state.store.fetches.contains(&FetchKey::Activity(42)));
    assert!(
        !app.state
            .store
            .reload_after_fetch
            .contains(&FetchKey::Activity(42))
    );
    app.apply(Action::Loaded(LoadedAction::Activity(
        42,
        Err("new response".into()),
    )));
    assert!(!app.state.store.fetches.contains(&FetchKey::Activity(42)));
    app.apply(Action::Loaded(LoadedAction::Mergeability(
        42,
        Err("old response".into()),
    )));
    app.apply(Action::Loaded(LoadedAction::Mergeability(
        42,
        Err("new response".into()),
    )));
    app.apply(Action::Loaded(LoadedAction::Prs(Err("offline".into()))));
    assert!(!app.state.store.refreshing(app.state.screen));
    assert!(matches!(app.state.store.cache.prs, LoadState::Loaded(_)));
}

#[test]
fn review_and_merge_dialogs_both_suspend_background_refresh() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    for action in [
        DetailAction::OpenReviewPicker,
        DetailAction::OpenMergePicker,
    ] {
        app.apply(Action::Detail(action));
        assert!(app.state.ui.modal_open(&app.state.store, app.state.screen));
        // A regression would attempt to spawn a provider task without a runtime.
        app.apply(Action::Refresh);
        assert!(app.state.store.fetches.is_empty());
        app.apply(Action::Detail(DetailAction::CloseReviewPicker));
        app.apply(Action::Detail(DetailAction::CloseMergePicker));
    }
}

#[test]
fn detail_emits_resolved_commands_and_retains_submission_payload() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Char('å'));
    let command = app.state.ui.update(
        Action::Detail(DetailAction::CommentSubmit),
        &app.state.store,
        app.state.screen,
    );
    assert!(matches!(command, Some(Action::Command {
        pr_id: 42, command: Command::SubmitComment { target: CommentTarget::Pr, text }
    }) if text == "å"));
    assert_eq!(app.state.ui.detail.editor.draft.as_ref().unwrap().text, "å");
    press(&mut app, KeyCode::Esc);
    app.apply(Action::Detail(DetailAction::OpenDecline));
    let command = app.state.ui.update(
        Action::Detail(DetailAction::SubmitConfirm),
        &app.state.store,
        app.state.screen,
    );
    assert!(matches!(
        command,
        Some(Action::Command {
            pr_id: 42,
            command: Command::Decline
        })
    ));
    assert!(app.state.ui.detail.confirm.is_none());
}
