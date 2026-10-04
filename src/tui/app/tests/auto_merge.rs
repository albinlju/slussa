//! Merging by itself once the PR is ready: `a` in the merge dialog.

use super::support::*;
use crate::{
    domain::pr::{MergeStrategy, Mergeability},
    tui::app::store::Operation,
};

fn blocked(app: &mut App) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.mergeability = LoadState::Loaded(Mergeability::Blocked(vec!["Checks.".into()]));
    }
}

#[tokio::test]
async fn a_then_enter_in_the_merge_dialog_asks_for_auto_merge_and_not_a_merge() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    blocked(&mut app);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('a'));
    assert!(
        app.state
            .ui
            .detail
            .merge_picker()
            .is_some_and(|d| d.when_ready)
    );
    press(&mut app, KeyCode::Enter);

    assert!(app.state.ui.detail.merge_picker().is_none());
    assert_eq!(
        app.state.store.operations.get(&PrId(42)),
        Some(&Operation::AutoMerge)
    );
}

#[tokio::test]
async fn a_is_a_toggle_and_enter_without_it_merges_as_before() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    blocked(&mut app);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Char('a'));
    assert!(
        app.state
            .ui
            .detail
            .merge_picker()
            .is_some_and(|d| !d.when_ready)
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.store.operations.get(&PrId(42)),
        Some(&Operation::Merge)
    );
}

#[tokio::test]
async fn a_on_a_pr_that_merges_by_itself_turns_that_off() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.mergeability = LoadState::Loaded(Mergeability::AutoMerge {
            strategy: MergeStrategy::Squash,
            waiting: vec![],
        });
    }
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(
        app.state.store.operations.get(&PrId(42)),
        Some(&Operation::CancelAutoMerge)
    );
}

#[tokio::test]
async fn a_does_nothing_where_the_pr_can_be_merged_now() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Char('a'));
    assert!(
        app.state
            .ui
            .detail
            .merge_picker()
            .is_some_and(|d| !d.when_ready)
    );
    assert!(app.state.store.operations.is_empty());
}
