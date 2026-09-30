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
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
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
            .filtered_prs(&app.state.store.cache.prs, &app.state.store.current_user)
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
        revision: None,
        path: "src/main.rs".into(),
        line: 1,
        removed: false,
    });
    press(&mut app, KeyCode::Char('c'));
    for c in "Please explain".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    send_comment(&mut app);
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
    assert!(!app.state.ui.detail.editor.is_open());
    assert_eq!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "qå"
    );
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
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err("offline".into()),
    }));
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
    send_comment(&mut app);
    assert!(app.state.store.operations.contains_key(&42));
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Backspace);
    send_comment(&mut app);
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
    send_comment(&mut app);
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
    send_comment(&mut app);
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
            submitted_summary: None,
            comments: vec![PendingComment {
                anchor: CommentAnchor {
                    revision: None,
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
    assert_eq!(app.state.store.fetches.len(), 4);
    assert!(app.state.store.refreshing(app.state.screen));
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err("offline".into()),
    }));
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
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err("offline".into()),
    }));
    app.apply(Action::Loaded(LoadedAction::Info(
        42,
        Err("offline".into()),
    )));
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
    app.apply(Action::Detail(DetailAction::ConfirmMove(-1)));
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

#[tokio::test(flavor = "current_thread")]
async fn escape_during_submission_keeps_request_and_draft_scoped() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Char('A'));
    send_comment(&mut app);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.screen, Screen::List);
    assert!(app.state.store.operations.contains_key(&42));
    app.apply(Action::Loaded(LoadedAction::Commented(
        42,
        Err("timeout".into()),
    )));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.ui.detail.editor.draft.as_ref().unwrap().text, "A");
    assert_eq!(app.state.detail_view().error(), Some("timeout"));
}

#[tokio::test(flavor = "current_thread")]
async fn partial_review_removes_confirmed_posts_and_remembers_sent_summary() {
    use crate::app::{
        reviews::{PendingComment, PendingReview},
        store::Operation,
    };
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    let comment = |text: &str| PendingComment {
        anchor: CommentAnchor {
            revision: None,
            path: "f".into(),
            line: 1,
            removed: false,
        },
        text: text.into(),
    };
    app.state.store.reviews.insert(
        42,
        PendingReview {
            submitted_summary: None,
            comments: vec![comment("sent"), comment("remaining")],
        },
    );
    app.state.store.operations.insert(42, Operation::Review);
    app.apply(Action::Loaded(LoadedAction::ReviewFailed {
        pr_id: 42,
        posted_comments: 1,
        submitted_summary: Some("summary".into()),
        message: "offline".into(),
    }));
    let review = &app.state.store.reviews[&42];
    assert_eq!(review.comments.len(), 1);
    assert_eq!(review.comments[0].text, "remaining");
    assert_eq!(review.submitted_summary.as_deref(), Some("summary"));
    assert!(!app.state.store.operations.contains_key(&42));
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_keys_open_the_latest_selection_and_capture_editor_text() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        let mut second = prs[0].clone();
        second.id = 43;
        let mut third = prs[0].clone();
        third.id = 44;
        prs.extend([second, third]);
    }
    for key in [
        KeyCode::Char('j'),
        KeyCode::Char('j'),
        KeyCode::Enter,
        KeyCode::Char('2'),
        KeyCode::Char('c'),
        KeyCode::Char('q'),
    ] {
        assert!(!app.handle_key(KeyEvent::new(key, KeyModifiers::NONE)));
    }
    assert!(matches!(app.state.screen, Screen::Detail { pr_id: 44, .. }));
    assert_eq!(app.state.ui.detail.editor.draft.as_ref().unwrap().text, "q");
}

#[test]
fn reply_shortcut_in_diff_opens_editor_for_the_focused_thread() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.state.ui.detail.diff.focus = DiffFocus::Pane;
    app.state.ui.detail.diff.pane_reply = Some(987);
    press(&mut app, KeyCode::Char('r'));
    assert!(matches!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().target,
        CommentTarget::Reply(987)
    ));
}

#[test]
fn read_only_capabilities_block_shortcuts_commands_and_optional_loads() {
    use crate::domain::{capabilities::Capabilities, pr::MergeStrategy, review::ReviewVerdict};
    let mut app = app();
    app.state.store.capabilities = Capabilities::default();
    let data = app.state.store.cache.details.get_mut(&42).unwrap();
    data.builds = LoadState::NotRequested;
    data.mergeability = LoadState::NotRequested;
    detail(&mut app, DetailTab::Overview);
    for ch in ['c', 'r', 'a', 'v', 'm', 'x', 'e', 'd', 'R', '5'] {
        press(&mut app, KeyCode::Char(ch));
    }
    assert!(app.state.ui.detail.editor.draft.is_none());
    assert!(app.state.ui.detail.review_picker.is_none());
    assert!(app.state.ui.detail.merge_picker.is_none());
    assert!(app.state.ui.detail.confirm.is_none());
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
    let data = &app.state.store.cache.details[&42];
    assert!(matches!(data.builds, LoadState::NotRequested));
    assert!(matches!(data.mergeability, LoadState::NotRequested));
    detail(&mut app, DetailTab::Commits);
    app.apply(Action::Detail(DetailAction::NextTab));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Description
        }
    );
    app.apply(Action::Detail(DetailAction::PrevTab));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Commits
        }
    );
    for command in [
        Command::StartReview,
        Command::SubmitReview {
            verdict: ReviewVerdict::Comment,
            body: "summary".into(),
        },
        Command::SubmitComment {
            target: CommentTarget::Pr,
            text: "comment".into(),
        },
        Command::Merge(MergeStrategy::Merge),
        Command::Decline,
        Command::DeleteComment {
            id: 1,
            review: false,
        },
        Command::ResolveThread {
            node_id: Some("thread".into()),
            comment_id: Some(1),
            resolved: true,
        },
    ] {
        app.state.store.errors.clear();
        app.execute(42, command);
        assert!(app.state.store.errors[&42].contains("not supported"));
        assert!(app.state.store.operations.is_empty());
        assert!(app.state.store.reviews.is_empty());
    }
}

#[test]
fn review_options_follow_capabilities_and_keep_own_pr_restrictions() {
    use crate::domain::review::ReviewVerdict;
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    assert!(
        !app.state
            .detail_view()
            .review_verdicts()
            .contains(&ReviewVerdict::Unapprove)
    );
    app.state
        .store
        .capabilities
        .review_verdicts
        .push(ReviewVerdict::Unapprove);
    assert!(
        app.state
            .detail_view()
            .review_verdicts()
            .contains(&ReviewVerdict::Unapprove)
    );
    app.state.store.current_user = "alice".into();
    let options = app.state.detail_view().review_context().options;
    assert!(options.contains(&(ReviewVerdict::Comment, None)));
    assert!(
        options
            .iter()
            .any(|(v, reason)| *v == ReviewVerdict::Approve && reason.is_some())
    );
    app.execute(
        42,
        Command::SubmitReview {
            verdict: ReviewVerdict::Approve,
            body: String::new(),
        },
    );
    assert!(app.state.store.operations.is_empty());
    assert!(app.state.store.errors[&42].contains("unavailable"));
}

#[test]
fn help_in_both_screens_captures_keys_and_restores_navigation() {
    for tab in [None, Some(DetailTab::Overview)] {
        let mut app = app();
        if let Some(tab) = tab {
            detail(&mut app, tab);
        }
        let screen = app.state.screen;
        press(&mut app, KeyCode::Char('?'));
        assert!(app.state.ui.modal_open(&app.state.store, screen));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 12)).unwrap();
        terminal.draw(|f| tui::render(f, &mut app.state)).unwrap();
        let selected = app.state.ui.list.selected;
        for ch in ['/', 'c', 'm', 'j'] {
            press(&mut app, KeyCode::Char(ch));
        }
        assert!(!app.state.ui.list.search.open);
        assert!(app.state.ui.detail.editor.draft.is_none());
        assert!(app.state.ui.detail.merge_picker.is_none());
        assert_eq!(app.state.ui.list.selected, selected);
        press(&mut app, KeyCode::Esc);
        assert!(!app.state.ui.modal_open(&app.state.store, screen));
        assert_eq!(app.state.screen, screen);
    }
}

#[test]
fn pending_review_can_be_finished_from_every_tab_that_shows_the_hint() {
    for tab in DetailTab::ALL {
        let mut app = app();
        detail(&mut app, tab);
        app.state.store.reviews.entry(42).or_default();
        press(&mut app, KeyCode::Char('v'));
        assert!(app.state.ui.detail.review_picker.is_some(), "{tab:?}");
    }
}

fn send_comment(app: &mut App) {
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
}

fn recovery_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "tuipr-recovery-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn attach_recovery(app: &mut App, root: &std::path::Path) {
    let (storage, snapshot) = super::drafts::reopen(root, "test-repo/reviewer").unwrap();
    app.restore_drafts(storage, snapshot);
}

#[test]
fn restart_restores_closed_editor_and_discard_removes_it_from_disk() {
    let root = recovery_root("editor");
    let mut first = app();
    attach_recovery(&mut first, &root);
    detail(&mut first, DetailTab::Overview);
    press(&mut first, KeyCode::Char('c'));
    first.apply(Action::Paste("first\r\nsecond 🦀".into()));
    press(&mut first, KeyCode::Enter);
    assert!(first.state.store.operations.is_empty());
    press(&mut first, KeyCode::Esc);
    drop(first);
    let mut second = app();
    attach_recovery(&mut second, &root);
    detail(&mut second, DetailTab::Description);
    assert!(!second.state.ui.detail.editor.is_open());
    press(&mut second, KeyCode::Char('c'));
    assert!(second.state.ui.detail.editor.is_open());
    assert_eq!(
        second.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "first\nsecond 🦀\n"
    );
    second.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
    press(&mut second, KeyCode::Enter);
    drop(second);
    let mut third = app();
    attach_recovery(&mut third, &root);
    detail(&mut third, DetailTab::Description);
    assert!(third.state.ui.detail.editor.draft.is_none());
    drop(third);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn interrupted_send_is_journaled_and_success_clears_recovery_data() {
    let root = recovery_root("send");
    let mut first = app();
    attach_recovery(&mut first, &root);
    detail(&mut first, DetailTab::Overview);
    press(&mut first, KeyCode::Char('c'));
    first.apply(Action::Paste("send me".into()));
    send_comment(&mut first);
    assert!(first.state.store.operations.contains_key(&42));
    drop(first);
    let mut second = app();
    attach_recovery(&mut second, &root);
    assert!(second.state.store.operations.is_empty());
    assert!(second.state.store.errors[&42].contains("may have reached"));
    detail(&mut second, DetailTab::Overview);
    press(&mut second, KeyCode::Esc); // acknowledge interrupted request notice
    press(&mut second, KeyCode::Char('c'));
    send_comment(&mut second);
    second.apply(Action::Loaded(LoadedAction::Commented(42, Ok(()))));
    drop(second);
    let mut third = app();
    attach_recovery(&mut third, &root);
    detail(&mut third, DetailTab::Overview);
    assert!(third.state.ui.detail.editor.draft.is_none());
    assert!(third.state.store.uncertain_submissions.is_empty());
    assert!(third.state.store.errors.is_empty());
    drop(third);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn journal_failure_prevents_remote_submission_and_keeps_editor() {
    let root = recovery_root("failure");
    let mut app = app();
    attach_recovery(&mut app, &root);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    app.apply(Action::Paste("keep me".into()));
    let file = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    std::fs::create_dir(file.with_extension("tmp")).unwrap();
    // No runtime: a remote spawn here would panic, so this verifies the boundary.
    send_comment(&mut app);
    assert!(app.state.store.operations.is_empty());
    assert_eq!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "keep me"
    );
    assert!(app.state.store.errors[&42].starts_with("Not sent:"));
    assert!(
        std::fs::read(&file)
            .unwrap()
            .windows(7)
            .any(|s| s == b"keep me")
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_link_is_rejected_before_starting_desktop_work() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].url = Some("file:///tmp/local".into());
    }
    app.apply(Action::PrLink {
        pr_id: 42,
        kind: LinkAction::Open,
    });
    assert!(!app.state.store.link_pending);
    assert!(app.state.store.notice.as_ref().unwrap().error);
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn link_completion_keeps_navigation_and_reports_failure_without_blocking_pr_work() {
    let mut app = app();
    app.apply(Action::PrLink {
        pr_id: 42,
        kind: LinkAction::Copy,
    });
    assert!(app.state.store.link_pending);
    detail(&mut app, DetailTab::Overview);
    app.apply(Action::LinkFinished(Err(
        "PR #42: clipboard unavailable".into()
    )));
    assert!(!app.state.store.link_pending);
    assert!(app.state.store.notice.as_ref().unwrap().error);
    assert!(app.state.store.errors.is_empty());
    assert!(app.state.store.operations.is_empty());
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
}

#[test]
fn failed_refresh_marks_cached_data_until_that_resource_recovers() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.apply(Action::Loaded(LoadedAction::Diff(
        42,
        Err("offline".into()),
    )));
    assert!(app.state.store.refresh_failed(app.state.screen));
    assert!(matches!(
        app.state.store.cache.details[&42].diff,
        LoadState::Loaded(_)
    ));
    assert!(!app.state.store.refresh_failed(Screen::Detail {
        pr_id: 43,
        tab: DetailTab::Diff
    }));
    app.apply(Action::Loaded(LoadedAction::Builds(42, Ok(vec![]))));
    assert!(app.state.store.refresh_failed(app.state.screen));
    let diff = match &app.state.store.cache.details[&42].diff {
        LoadState::Loaded(diff) => diff.clone(),
        _ => unreachable!(),
    };
    app.apply(Action::Loaded(LoadedAction::Diff(42, Ok(diff))));
    assert!(!app.state.store.refresh_failed(app.state.screen));
}

#[test]
fn discarding_a_populated_review_requires_explicit_confirmation() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state.store.reviews.insert(
        42,
        crate::app::reviews::PendingReview {
            comments: vec![crate::app::reviews::PendingComment {
                anchor: CommentAnchor {
                    revision: None,
                    path: "src/main.rs".into(),
                    line: 1,
                    removed: false,
                },
                text: "Keep this draft".into(),
            }],
            submitted_summary: None,
        },
    );
    press(&mut app, KeyCode::Char('V'));
    assert_eq!(
        app.state.ui.detail.confirm.as_ref().unwrap().kind(),
        ConfirmKind::DiscardReview
    );
    // Enter defaults to keeping the review, not removing it.
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.store.reviews[&42].comments.len(), 1);
    press(&mut app, KeyCode::Char('V'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.store.reviews[&42].comments.len(), 1);
    press(&mut app, KeyCode::Char('V'));
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    assert!(!app.state.store.reviews.contains_key(&42));
}

#[test]
fn comment_lookup_keeps_review_and_pr_ids_separate() {
    use crate::domain::{
        activity::Activity,
        comment::{Comment, CommentThread, ThreadAnchor},
        user::User,
    };
    let mut app = app();
    app.state.store.current_user = "reviewer".into();
    detail(&mut app, DetailTab::Overview);
    let comment = |author: &str, text: &str| Comment {
        id: Some(7),
        author: User {
            username: author.into(),
        },
        content: text.into(),
        created: chrono::Utc::now(),
        reactions: vec![],
        reply_to: None,
    };
    app.state.store.cache.details.get_mut(&42).unwrap().activity = LoadState::Loaded(Activity {
        comments: vec![comment("other", "PR comment")],
        events: vec![],
        threads: vec![CommentThread {
            comments: vec![comment("reviewer", "My review comment")],
            reply_to: Some(7),
            anchor: Some(ThreadAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: Some(1),
                old_line: None,
                resolved: false,
                node_id: Some("thread".into()),
            }),
        }],
    });
    app.state.ui.detail.overview.timeline.selected =
        Some(tui::screens::pr_detail::view::CommentRef {
            id: Some(7),
            review: true,
        });
    press(&mut app, KeyCode::Char('e'));
    let draft = app.state.ui.detail.editor.draft.as_ref().unwrap();
    assert_eq!(draft.text, "My review comment");
    assert!(matches!(
        draft.target,
        CommentTarget::Edit {
            id: 7,
            review: true
        }
    ));
    app.state.ui.detail.editor = tui::components::comment_editor::CommentEditor::default();
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(
        app.state.ui.detail.confirm.as_ref().unwrap().kind(),
        ConfirmKind::DeleteComment {
            id: 7,
            review: true
        }
    );
    press(&mut app, KeyCode::Esc);
    app.state
        .ui
        .detail
        .overview
        .timeline
        .selected
        .as_mut()
        .unwrap()
        .review = false;
    press(&mut app, KeyCode::Char('e'));
    assert!(app.state.ui.detail.editor.draft.is_none());
}

#[test]
fn refreshed_lists_keep_pr_and_commit_identity() {
    let mut app = app();
    let LoadState::Loaded(mut prs) = std::mem::take(&mut app.state.store.cache.prs) else {
        panic!()
    };
    let mut second = prs[0].clone();
    second.id = 43;
    prs.push(second);
    app.state.store.cache.prs = LoadState::Loaded(prs.clone());
    app.state.ui.list.selected = 1;
    let mut new = prs[0].clone();
    new.id = 44;
    prs.insert(0, new);
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Ok(crate::domain::pr::PrBatch { prs, more: None }),
    }));
    assert_eq!(app.state.ui.list.selected, 2);
    detail(&mut app, DetailTab::Commits);
    let data = app.state.store.cache.details.get_mut(&42).unwrap();
    let LoadState::Loaded(mut commits) = std::mem::take(&mut data.commits) else {
        panic!()
    };
    let mut second = commits[0].clone();
    second.oid = "second".into();
    commits.push(second);
    data.commits = LoadState::Loaded(commits.clone());
    app.state.ui.detail.commits.selected = 1;
    let mut new = commits[0].clone();
    new.oid = "new".into();
    commits.insert(0, new);
    app.apply(Action::Loaded(LoadedAction::Commits(42, Ok(commits))));
    assert_eq!(app.state.ui.detail.commits.selected, 2);
}

#[test]
fn returning_to_a_pr_restores_its_tab_focus_and_search() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.state.ui.detail.diff.focus = DiffFocus::Pane;
    app.state.ui.detail.diff.pane_search.query = "needle".into();
    app.state.ui.detail.diff.pane_scroll = 12;
    app.state.ui.detail.diff.pane_cursor = 5;
    app.state.ui.detail.overview.timeline.scroll = 7;
    let mut other = tui::regression_tests::fixture();
    app.state
        .store
        .cache
        .details
        .insert(43, other.store.cache.details.remove(&42).unwrap());
    app.apply(Action::Navigate(Screen::List));
    app.apply(Action::List(ListAction::OpenPr(43)));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 43,
            tab: DetailTab::Description
        }
    );
    app.apply(Action::List(ListAction::OpenPr(42)));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Diff
        }
    );
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Pane);
    assert_eq!(app.state.ui.detail.diff.pane_search.query, "needle");
    assert_eq!(app.state.ui.detail.diff.pane_scroll, 12);
    assert_eq!(app.state.ui.detail.overview.timeline.scroll, 7);
}

#[tokio::test(flavor = "current_thread")]
async fn successful_mutation_reports_which_pr_changed() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state
        .store
        .operations
        .insert(42, super::store::Operation::Merge);
    app.apply(Action::Loaded(LoadedAction::Merged(42, Ok(()))));
    let notice = app.state.store.notice.as_ref().unwrap();
    assert!(!notice.error);
    assert!(notice.message.contains("42"));
    assert!(notice.message.contains("merged"));
}
