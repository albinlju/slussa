//! Which columns the list shows: the status where the views mix, and a conflict
//! wherever there is one.

use super::support::*;
use crate::tui::ui::screens::pr_list::StatusFilter;

fn list(filter: StatusFilter, conflicting: bool) -> AppState {
    let mut state = fixture();
    state.screen = Screen::List;
    state.ui.list.filter = filter;
    if let LoadState::Loaded(prs) = &mut state.store.cache.prs {
        prs[0].has_conflicts = conflicting;
    }
    state
}

#[test]
fn the_status_column_is_there_where_the_views_mix_and_where_a_pr_has_a_conflict() {
    let all = draw(&mut list(StatusFilter::All, false), 130, 12);
    assert!(all.contains("Status"), "{all}");
    for filter in [
        StatusFilter::Open,
        StatusFilter::Merged,
        StatusFilter::Declined,
    ] {
        let quiet = draw(&mut list(filter, false), 130, 12);
        assert!(!quiet.contains("Status"), "{filter:?}: {quiet}");
    }
    let conflicting = draw(&mut list(StatusFilter::Open, true), 130, 12);
    assert!(
        conflicting.contains("Status") && conflicting.contains("conflicts"),
        "{conflicting}"
    );
}

#[test]
fn a_conflict_is_named_for_every_reader_and_not_among_the_reasons() {
    for viewer in ["alice", "someone-else"] {
        let mut state = list(StatusFilter::Open, true);
        state.store.current_user = viewer.into();
        let text = draw(&mut state, 130, 12);
        assert!(text.contains("conflicts"), "{viewer}: {text}");
        assert!(!text.contains("Needs you"), "{viewer}: {text}");
    }
}

#[test]
fn in_the_mixed_view_a_conflict_stands_where_the_status_would() {
    let text = draw(&mut list(StatusFilter::All, true), 130, 12);
    assert!(
        text.contains("conflicts") && !text.contains("Open"),
        "{text}"
    );
    let plain = draw(&mut list(StatusFilter::All, false), 130, 12);
    assert!(
        plain.contains("Open") && !plain.contains("conflicts"),
        "{plain}"
    );
}
