//! Writes in flight: what a result clears, keeps or reports, and for which PR.

use super::support::*;

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
    assert!(app.state.store.operations.contains_key(&PrId(42)));
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Backspace);
    send_comment(&mut app);
    assert_eq!(app.state.store.operations.len(), 1);
    assert_eq!(
        app.state.ui.detail.editor.text().unwrap(),
        "Keep this draft"
    );
    // A read result must never acknowledge a write.
    app.apply_result(TaskResult::Read(Read::Activity(
        PrId(42),
        Err(failed("read failed")),
    )));
    assert!(app.state.store.operations.contains_key(&PrId(42)));
    finish_write(&mut app, PrId(42), Err(failed("offline").into()));
    assert!(!app.state.store.operations.contains_key(&PrId(42)));
    assert_eq!(
        app.state.store.errors.get(&PrId(42)).map(String::as_str),
        Some("offline")
    );
    press(&mut app, KeyCode::Enter); // dismiss error, do not resubmit
    assert!(!app.state.store.operations.contains_key(&PrId(42)));
    assert!(app.state.store.errors.is_empty());
    assert_eq!(
        app.state.ui.detail.editor.text().unwrap(),
        "Keep this draft"
    );
    send_comment(&mut app);
    finish_write(&mut app, PrId(42), Ok(()));
    assert!(!app.state.ui.detail.editor.has_draft());
}

#[tokio::test(flavor = "current_thread")]
async fn late_completion_only_clears_the_submitting_pr_editor() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        let mut second = prs[0].clone();
        second.id = PrId(43);
        prs.push(second);
    }
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Char('A'));
    send_comment(&mut app);
    add_pr(&mut app, 43);
    app.open_pr(PrId(43));
    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(
        DetailTab::Overview,
    ))));
    app.apply(Action::Detail(DetailAction::Pr(PrAction::OpenComment)));
    app.apply(Action::Detail(DetailAction::Editor(EditorAction::Type(
        'B',
    ))));
    finish_write(&mut app, PrId(42), Ok(()));
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "B");
    app.open_pr(PrId(42));
    assert!(!app.state.ui.detail.editor.has_draft());
    app.open_pr(PrId(43));
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "B");
}

#[tokio::test(flavor = "current_thread")]
async fn failed_review_and_error_stay_with_their_pr_until_success() {
    use crate::{
        domain::review::ReviewVerdict,
        tui::app::{
            reviews::{PendingComment, PendingReview},
            store::Operation,
        },
    };
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state.store.reviews.insert(
        PrId(42),
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
    app.apply(Action::Effect(Effect::Command {
        pr_id: PrId(42),
        command: Command::SubmitReview {
            verdict: ReviewVerdict::Approve,
            body: String::new(),
        },
    }));
    assert_eq!(app.state.store.reviews[&PrId(42)].comments.len(), 1);
    add_pr(&mut app, 43);
    app.open_pr(PrId(43));
    app.state
        .store
        .operations
        .insert(PrId(43), Operation::Merge);
    finish_write(&mut app, PrId(42), Err(failed("offline").into()));
    assert!(app.state.store.operations.contains_key(&PrId(43)));
    assert!(app.state.detail_view().error().is_none());
    assert_eq!(app.state.store.reviews[&PrId(42)].comments.len(), 1);
    app.open_pr(PrId(42));
    assert_eq!(app.state.detail_view().error(), Some("offline"));
    press(&mut app, KeyCode::Esc);
    app.apply(Action::Effect(Effect::Command {
        pr_id: PrId(42),
        command: Command::SubmitReview {
            verdict: ReviewVerdict::Approve,
            body: String::new(),
        },
    }));
    finish_write(&mut app, PrId(42), Ok(()));
    assert!(!app.state.store.reviews.contains_key(&PrId(42)));
    assert!(app.state.store.operations.contains_key(&PrId(43)));
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
    assert!(app.state.store.operations.contains_key(&PrId(42)));
    finish_write(&mut app, PrId(42), Err(failed("timeout").into()));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "A");
    assert_eq!(app.state.detail_view().error(), Some("timeout"));
}

#[tokio::test(flavor = "current_thread")]
async fn partial_review_removes_confirmed_posts_and_remembers_sent_summary() {
    use crate::tui::app::{
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
        PrId(42),
        PendingReview {
            submitted_summary: None,
            comments: vec![comment("sent"), comment("remaining")],
        },
    );
    app.state
        .store
        .operations
        .insert(PrId(42), Operation::Review);
    finish_write(
        &mut app,
        PrId(42),
        Err(WriteError::PartialReview {
            posted_comments: 1,
            submitted_summary: Some("summary".into()),
            source: failed("offline"),
        }),
    );
    let review = &app.state.store.reviews[&PrId(42)];
    assert_eq!(review.comments.len(), 1);
    assert_eq!(review.comments[0].text, "remaining");
    assert_eq!(review.submitted_summary.as_deref(), Some("summary"));
    assert!(!app.state.store.operations.contains_key(&PrId(42)));
}

#[tokio::test(flavor = "current_thread")]
async fn successful_mutation_reports_which_pr_changed() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state
        .store
        .operations
        .insert(PrId(42), crate::tui::app::store::Operation::Merge);
    finish_write(&mut app, PrId(42), Ok(()));
    let notice = app.state.store.notice.as_ref().unwrap();
    assert_eq!(notice.kind, crate::tui::app::store::NoticeKind::Info);
    assert!(notice.message.contains("42"));
    assert!(notice.message.contains("merged"));
}
