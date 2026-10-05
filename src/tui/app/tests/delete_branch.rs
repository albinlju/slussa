//! Deleting the source branch with the merge: `d` in the merge dialog.

use super::support::*;
use crate::domain::pr::{DeletableBranch, MergeStrategy, SourceRepo};

fn own_branch(app: &mut App) {
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].source_repo = SourceRepo::Same;
    }
}

fn enter(app: &mut App) -> Option<Effect> {
    app.state.ui.update(
        Action::Detail(DetailAction::Merge(MergeAction::Select)),
        &app.state.store,
        app.state.screen,
    )
}

#[tokio::test]
async fn d_marks_the_branch_and_enter_merges_with_it_marked() {
    let mut app = app();
    own_branch(&mut app);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('d'));
    assert!(
        app.state
            .ui
            .detail
            .merge_picker()
            .is_some_and(|d| d.delete_branch)
    );

    let Some(Effect::Command {
        command: Command::Merge { strategy, delete },
        ..
    }) = enter(&mut app)
    else {
        panic!("Enter merges");
    };
    assert_eq!(strategy, MergeStrategy::Merge);
    assert_eq!(delete.as_ref().map(DeletableBranch::name), Some("feature"));
}

#[tokio::test]
async fn d_is_a_toggle_and_enter_without_it_keeps_the_branch() {
    let mut app = app();
    own_branch(&mut app);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('d'));

    let Some(Effect::Command {
        command: Command::Merge { delete, .. },
        ..
    }) = enter(&mut app)
    else {
        panic!("Enter merges");
    };
    assert_eq!(delete, None);
}

#[tokio::test]
async fn d_does_nothing_where_the_branch_is_not_the_repositorys_own_to_delete() {
    let mut app = app();
    // The fixture's source repository is not known, as on Bitbucket.
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('d'));
    assert!(
        app.state
            .ui
            .detail
            .merge_picker()
            .is_some_and(|d| !d.delete_branch)
    );

    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].source_repo = SourceRepo::Fork;
    }
    press(&mut app, KeyCode::Char('d'));
    let Some(Effect::Command {
        command: Command::Merge { delete, .. },
        ..
    }) = enter(&mut app)
    else {
        panic!("Enter merges");
    };
    assert_eq!(delete, None);
}

#[tokio::test]
async fn a_merge_that_kept_its_branch_is_done_and_says_so() {
    let mut app = app();
    own_branch(&mut app);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Enter);
    finish_write(
        &mut app,
        PrId(42),
        Err(WriteError::BranchKept(failed("Reference does not exist"))),
    );

    let notice = app.state.store.notice.as_ref().map(|n| n.message.as_str());
    assert!(
        notice.is_some_and(|n| n.contains("merged, but the branch was not deleted")),
        "{notice:?}"
    );
    assert!(app.state.store.errors.is_empty(), "no error dialog");
    assert!(app.state.store.uncertain_submissions.is_empty());
    assert!(app.state.store.operations.is_empty());
}
