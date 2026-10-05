//! A merge and a verdict are tied to the commit the reader has seen.

use super::support::*;
use crate::domain::{
    diff::{Diff, DiffRevision},
    pr::MergeStrategy,
};

fn enter(app: &mut App) -> Option<Effect> {
    app.state.ui.update(
        Action::Detail(DetailAction::Merge(MergeAction::Select)),
        &app.state.store,
        app.state.screen,
    )
}

fn listed_head(app: &mut App, head: Option<&str>) {
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].head_oid = head.map(str::to_owned);
    }
}

fn read_diff_at(app: &mut App, head: &str) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.diff = LoadState::Loaded(Diff {
            revision: Some(DiffRevision {
                head: head.into(),
                base: None,
                commit: false,
            }),
            files: vec![],
        });
    }
}

fn merge_command(app: &mut App) -> (MergeStrategy, String) {
    let Some(Effect::Command {
        command: Command::Merge { strategy, head, .. },
        ..
    }) = enter(app)
    else {
        panic!("Enter merges");
    };
    (strategy, head.as_str().to_owned())
}

#[tokio::test]
async fn a_merge_is_tied_to_the_head_the_list_gave_when_no_diff_was_read() {
    let mut app = app();
    listed_head(&mut app, Some("abc123"));
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    assert_eq!(merge_command(&mut app).1, "abc123");
}

#[tokio::test]
async fn a_merge_is_tied_to_the_head_of_the_diff_that_was_read_not_a_newer_listed_one() {
    let mut app = app();
    listed_head(&mut app, Some("beef01"));
    detail(&mut app, DetailTab::Overview);
    read_diff_at(&mut app, "cafe02");
    press(&mut app, KeyCode::Char('m'));
    assert_eq!(
        merge_command(&mut app).1,
        "cafe02",
        "the author pushed after the diff was read: GitHub will refuse"
    );
}

#[tokio::test]
async fn without_a_known_head_merge_and_review_are_not_offered() {
    let mut app = app();
    listed_head(&mut app, None);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    assert!(app.state.ui.detail.merge_picker().is_none());
    press(&mut app, KeyCode::Char('a'));
    assert!(app.state.ui.detail.review_picker().is_none());
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_verdict_draft_whose_head_went_missing_says_so_and_keeps_the_editor_open() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('a'));
    // Request changes needs a summary: the editor opens.
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert!(
        app.state.ui.detail.editor.is_open(),
        "the summary editor is open"
    );
    press(&mut app, KeyCode::Char('z'));
    // The list is read again and gives no head.
    listed_head(&mut app, None);
    send_comment(&mut app);

    let error = app.state.store.errors.get(&PrId(42));
    assert!(
        error.is_some_and(|e| e.contains("commit you are reviewing is not known")),
        "{error:?}"
    );
    assert!(app.state.store.operations.is_empty());
    assert!(app.state.ui.detail.editor.is_open(), "the draft is kept");
}
