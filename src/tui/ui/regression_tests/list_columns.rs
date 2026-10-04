//! Which columns the list shows: the status only where the views are mixed, and
//! the reason a conflict gives.

use super::support::*;
use crate::tui::ui::screens::pr_list::StatusFilter;

fn list(filter: StatusFilter) -> AppState {
    let mut state = fixture();
    state.screen = Screen::List;
    state.ui.list.filter = filter;
    state
}

#[test]
fn the_status_column_is_there_only_in_the_view_that_mixes_statuses() {
    let all = draw(&mut list(StatusFilter::All), 130, 12);
    assert!(all.contains("Status"), "{all}");
    for filter in [
        StatusFilter::Open,
        StatusFilter::Merged,
        StatusFilter::Declined,
    ] {
        let text = draw(&mut list(filter), 130, 12);
        assert!(!text.contains("Status"), "{filter:?}: {text}");
    }
}

#[test]
fn a_conflict_on_ones_own_pr_is_named_in_the_needs_you_column() {
    let mut state = list(StatusFilter::Open);
    let LoadState::Loaded(prs) = &mut state.store.cache.prs else {
        panic!("the fixture's list is loaded");
    };
    prs[0].has_conflicts = true;
    state.store.current_user = prs[0].author.username.as_str().into();
    let text = draw(&mut state, 130, 12);
    assert!(
        text.contains("Needs you") && text.contains("conflicts"),
        "{text}"
    );

    // Someone else looking at the same PR sees no reason.
    state.store.current_user = "someone-else".into();
    assert!(!draw(&mut state, 130, 12).contains("conflicts"));
}
