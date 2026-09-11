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
    assert!(!app.state.ui.detail.comment_pending);
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
