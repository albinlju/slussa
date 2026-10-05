//! What a PR's status says once a write has changed it.

use super::support::*;
use crate::{domain::pr::PrStatus, tui::app::store::Operation};

fn status_of(app: &App, id: u64) -> Option<PrStatus> {
    app.state
        .store
        .cache
        .prs
        .loaded()?
        .iter()
        .find(|pr| pr.id == PrId(id))
        .map(|pr| pr.status.clone())
}

#[tokio::test]
async fn a_merged_pr_is_not_shown_with_the_conflict_it_had() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        for pr in prs.iter_mut().filter(|pr| pr.id == PrId(42)) {
            pr.status = PrStatus::conflicting();
        }
    }
    assert!(status_of(&app, 42).is_some_and(|status| status.has_conflicts()));

    app.state
        .store
        .operations
        .insert(PrId(42), Operation::Merge);
    finish_write(&mut app, PrId(42), Ok(()));

    assert_eq!(status_of(&app, 42), Some(PrStatus::Merged));
    assert!(!status_of(&app, 42).is_some_and(|status| status.has_conflicts()));
}
