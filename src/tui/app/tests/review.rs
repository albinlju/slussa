//! Review drafts, verdicts, comment targets and the dialogs that gate them.

use super::support::*;

#[test]
fn queued_review_survives_navigation_without_crossing_prs() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    press(&mut app, KeyCode::Char('v'));
    app.state.ui.detail.diff.focus = DiffFocus::Pane;
    app.state.ui.detail.diff.pane.focused = Some(FocusedNav::on(NavTarget::Line));
    press(&mut app, KeyCode::Char('c'));
    for c in "Please explain".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    send_comment(&mut app);
    assert_eq!(
        app.state.store.reviews[&PrId(42)].comments[0].text,
        "Please explain"
    );
    // Opening another PR uses already cached data and never contacts a provider.
    let data = app.state.store.cache.details.remove(&PrId(42)).unwrap();
    app.state.store.cache.details.insert(PrId(43), data);
    add_pr(&mut app, 43);
    app.apply(Action::List(ListAction::OpenPr(PrId(43))));
    assert!(app.state.detail_view().pending_review().is_none());
    app.apply(Action::Detail(DetailAction::Pr(PrAction::StartReview)));
    assert!(app.state.store.reviews[&PrId(43)].comments.is_empty());
    app.apply(Action::Detail(DetailAction::Pr(PrAction::AbandonReview)));
    assert_eq!(app.state.store.reviews[&PrId(42)].comments.len(), 1);
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
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "qå");
    press(&mut app, KeyCode::Esc);
    assert!(!app.state.ui.detail.editor.is_open());
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "qå");
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
            .review_picker()
            .map(ReviewDialog::cursor),
        Some(2)
    );
    press(&mut app, KeyCode::Enter);
    assert!(matches!(
        app.state.ui.detail.editor.target().unwrap(),
        CommentTarget::Review {
            verdict: crate::domain::review::ReviewVerdict::Comment
        }
    ));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('x'));
    assert_eq!(
        app.state.ui.detail.confirm().map(ConfirmDialog::kind),
        Some(ConfirmKind::Decline)
    );
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Enter);
    assert!(app.state.ui.detail.confirm().is_none());
    assert!(!app.state.store.operations.contains_key(&PrId(42)));
}

#[test]
fn review_picker_captures_keys_before_diff_search() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    press(&mut app, KeyCode::Char('v'));
    press(&mut app, KeyCode::Char('v'));
    assert!(app.state.ui.detail.review_picker().is_some());
    press(&mut app, KeyCode::Char('/'));
    assert!(!app.state.ui.detail.diff.tree_search.open);
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.review_picker().is_none());
    assert!(app.state.store.reviews.contains_key(&PrId(42)));
}

#[test]
fn review_and_merge_dialogs_both_suspend_background_refresh() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    for action in [
        DetailAction::Pr(PrAction::OpenReviewPicker),
        DetailAction::Pr(PrAction::OpenMergePicker),
    ] {
        app.apply(Action::Detail(action));
        assert!(app.state.ui.modal_open(&app.state.store, app.state.screen));
        // A regression would attempt to spawn a provider task without a runtime.
        app.apply(Action::Effect(Effect::Refresh));
        assert!(app.state.store.fetches.is_empty());
        app.apply(Action::Detail(DetailAction::Review(ReviewAction::Close)));
        app.apply(Action::Detail(DetailAction::Merge(MergeAction::Close)));
    }
}

#[test]
fn detail_emits_resolved_commands_and_retains_submission_payload() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Char('å'));
    let command = app.state.ui.update(
        Action::Detail(DetailAction::Editor(EditorAction::Submit)),
        &app.state.store,
        app.state.screen,
    );
    assert!(matches!(command, Some(Effect::Command {
        pr_id: PrId(42), command: Command::SubmitComment { target: CommentTarget::Pr, text }
    }) if text.as_str() == "å"));
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "å");
    press(&mut app, KeyCode::Esc);
    app.apply(Action::Detail(DetailAction::Pr(PrAction::OpenDecline)));
    app.apply(Action::Detail(DetailAction::Confirm(ConfirmAction::Move(
        -1,
    ))));
    let command = app.state.ui.update(
        Action::Detail(DetailAction::Confirm(ConfirmAction::Accept)),
        &app.state.store,
        app.state.screen,
    );
    assert!(matches!(
        command,
        Some(Effect::Command {
            pr_id: PrId(42),
            command: Command::Decline
        })
    ));
    assert!(app.state.ui.detail.confirm().is_none());
}

#[test]
fn reply_shortcut_in_diff_opens_editor_for_the_focused_thread() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.state.ui.detail.diff.focus = DiffFocus::Pane;
    app.state.ui.detail.diff.pane.focused = Some(FocusedNav::on_thread(CommentId(987)));
    press(&mut app, KeyCode::Char('r'));
    assert!(matches!(
        app.state.ui.detail.editor.target().unwrap(),
        CommentTarget::Reply(CommentId(987))
    ));
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
        .review
        .as_mut()
        .unwrap()
        .verdicts
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
        PrId(42),
        Command::SubmitReview {
            verdict: ReviewVerdict::Approve,
            body: String::new(),
        },
    );
    assert!(app.state.store.operations.is_empty());
    assert!(app.state.store.errors[&PrId(42)].contains("unavailable"));
}

#[test]
fn pending_review_can_be_finished_from_every_tab_that_shows_the_hint() {
    for tab in DetailTab::ALL {
        let mut app = app();
        detail(&mut app, tab);
        app.state.store.reviews.entry(PrId(42)).or_default();
        press(&mut app, KeyCode::Char('v'));
        assert!(app.state.ui.detail.review_picker().is_some(), "{tab:?}");
    }
}

#[test]
fn discarding_a_populated_review_requires_explicit_confirmation() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state.store.reviews.insert(
        PrId(42),
        crate::tui::app::reviews::PendingReview {
            comments: vec![crate::tui::app::reviews::PendingComment {
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
        app.state.ui.detail.confirm().unwrap().kind(),
        ConfirmKind::DiscardReview
    );
    // Enter defaults to keeping the review, not removing it.
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.store.reviews[&PrId(42)].comments.len(), 1);
    press(&mut app, KeyCode::Char('V'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.store.reviews[&PrId(42)].comments.len(), 1);
    press(&mut app, KeyCode::Char('V'));
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    assert!(!app.state.store.reviews.contains_key(&PrId(42)));
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
        id: Some(CommentId(7)),
        author: User {
            username: author.into(),
        },
        account: crate::domain::user::AccountKind::Person,
        authorship: crate::domain::authorship::Authorship::Human,
        content: text.into(),
        created: chrono::Utc::now(),
        reactions: vec![],
        reply_to: None,
    };
    app.state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![comment("other", "PR comment")],
        events: vec![],
        threads: vec![CommentThread {
            comments: vec![comment("reviewer", "My review comment")],
            reply_to: Some(CommentId(7)),
            anchor: Some(ThreadAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: Some(crate::domain::diff::LineRef::New(1)),
                resolved: false,
                handle: Some(ThreadHandle::NodeId("thread".into())),
            }),
        }],
    });
    app.state.ui.detail.overview.timeline.selected = Some(
        ui::screens::pr_detail::view::CommentRef::new(Some(CommentId(7)), CommentKind::Review),
    );
    press(&mut app, KeyCode::Char('e'));
    let draft = app.state.ui.detail.editor.draft().unwrap();
    assert_eq!(draft.text, "My review comment");
    assert!(matches!(
        draft.target,
        CommentTarget::Edit(CommentKey {
            id: CommentId(7),
            kind: CommentKind::Review
        })
    ));
    app.state.ui.detail.editor = ui::components::comment_editor::CommentEditor::default();
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(
        app.state.ui.detail.confirm().unwrap().kind(),
        ConfirmKind::DeleteComment(CommentKey {
            id: CommentId(7),
            kind: CommentKind::Review
        })
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
        .kind = CommentKind::Conversation;
    press(&mut app, KeyCode::Char('e'));
    assert!(!app.state.ui.detail.editor.has_draft());
}
