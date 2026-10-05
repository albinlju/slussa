//! Asking those who asked for changes to review again: `p`.

use super::support::*;
use crate::domain::{
    pr::PrStatus,
    review::{Reviewer, ReviewerState},
    user::User,
};
use crate::tui::app::store::Operation;

fn reviewers(app: &mut App, states: &[(&str, ReviewerState)]) {
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].reviewers = states
            .iter()
            .map(|(name, state)| Reviewer {
                author: User {
                    username: (*name).into(),
                },
                state: state.clone(),
            })
            .collect();
    }
}

#[tokio::test]
async fn p_asks_again_once_somebody_asked_for_changes() {
    let mut app = app();
    reviewers(
        &mut app,
        &[
            ("alice", ReviewerState::ChangesRequested),
            ("bob", ReviewerState::Approved),
        ],
    );
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('p'));

    assert_eq!(
        app.state.store.operations.get(&PrId(42)),
        Some(&Operation::RerequestReview)
    );
}

#[tokio::test]
async fn p_does_nothing_when_nobody_asked_for_changes() {
    let mut app = app();
    reviewers(
        &mut app,
        &[
            ("bob", ReviewerState::Approved),
            ("carol", ReviewerState::Requested),
        ],
    );
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('p'));

    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn p_is_blocked_on_a_pr_that_is_not_open() {
    let mut app = app();
    reviewers(&mut app, &[("alice", ReviewerState::ChangesRequested)]);
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].status = PrStatus::Merged;
    }
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('p'));

    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_finished_request_is_a_notice_and_the_list_is_read_again() {
    let mut app = app();
    reviewers(&mut app, &[("alice", ReviewerState::ChangesRequested)]);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('p'));
    finish_write(&mut app, PrId(42), Ok(()));

    let notice = app.state.store.notice.as_ref().map(|n| n.message.as_str());
    assert_eq!(notice, Some("PR #42 · asked to review again"));
    assert!(app.state.store.operations.is_empty());
}
