//! The sort picker, and the orders it picks between.

use super::support::*;
use crate::tui::{
    app::pr_groups::OpenChain,
    ui::screens::pr_list::{ListOverlay, Sort},
};
use chrono::Duration;

/// Three PRs in the provider's order (newest created first), whose last updates
/// are in another order: #1 is the oldest but was touched just now, #3 the newest
/// and untouched since it was made.
fn list() -> AppState {
    let mut state = fixture();
    state.screen = Screen::List;
    let LoadState::Loaded(prs) = &mut state.store.cache.prs else {
        panic!("the fixture's list is loaded");
    };
    let base = prs[0].clone();
    let made = |id: u64, created_hours_ago: i64, updated_hours_ago: i64| {
        let mut pr = base.clone();
        pr.id = PrId(id);
        pr.title = format!("Change {id}");
        pr.author.username = "someone".into();
        pr.reviewers.clear();
        pr.created = chrono::Utc::now() - Duration::hours(created_hours_ago);
        pr.updated = chrono::Utc::now() - Duration::hours(updated_hours_ago);
        pr
    };
    *prs = vec![made(3, 1, 1), made(2, 5, 4), made(1, 20, 0)];
    state
}

/// The PR numbers in the order the list shows them.
fn order(state: &mut AppState) -> Vec<u64> {
    let text = draw(state, 140, 14);
    let mut found: Vec<(usize, u64)> = [1, 2, 3]
        .into_iter()
        .filter_map(|id| text.find(&format!("Change {id}")).map(|at| (at, id)))
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, id)| id).collect()
}

fn pick(state: &mut AppState, sort: Sort) {
    state.ui.list.sort = sort;
}

#[test]
fn each_sort_orders_the_rows_by_what_it_names() {
    let mut state = list();
    pick(&mut state, Sort::Recent);
    assert_eq!(order(&mut state), [3, 2, 1], "newest created first");
    pick(&mut state, Sort::Updated);
    assert_eq!(order(&mut state), [1, 3, 2], "latest activity first");
    pick(&mut state, Sort::Oldest);
    assert_eq!(order(&mut state), [1, 2, 3], "oldest created first");
}

#[test]
fn while_the_open_pages_are_still_coming_in_no_sort_moves_the_rows() {
    let mut state = list();
    state.store.open_chain = OpenChain::Appending;
    for sort in Sort::CYCLE {
        pick(&mut state, sort);
        assert_eq!(order(&mut state), [3, 2, 1], "{sort:?}");
    }
}

#[test]
fn s_opens_a_picker_with_the_four_sorts_and_enter_applies_the_one_chosen() {
    let mut state = list();
    local_key(&mut state, KeyCode::Char('s'));
    assert!(matches!(
        state.ui.list.overlay,
        Some(ListOverlay::SortPicker {
            highlighted: Sort::Attention
        })
    ));
    let text = draw(&mut state, 100, 24);
    for label in [
        "Sort",
        "Needs you first",
        "Newest",
        "Recently updated",
        "Oldest",
    ] {
        assert!(text.contains(label), "{label}: {text}");
    }
    // Down to Recently updated, then Enter.
    local_key(&mut state, KeyCode::Char('j'));
    local_key(&mut state, KeyCode::Char('j'));
    local_key(&mut state, KeyCode::Enter);
    assert!(state.ui.list.overlay.is_none());
    assert_eq!(state.ui.list.sort, Sort::Updated);
    assert_eq!(order(&mut state), [1, 3, 2]);
}

#[test]
fn esc_or_s_leaves_the_picker_without_choosing() {
    for leave in [KeyCode::Esc, KeyCode::Char('s')] {
        let mut state = list();
        local_key(&mut state, KeyCode::Char('s'));
        local_key(&mut state, KeyCode::Char('j'));
        local_key(&mut state, leave);
        assert!(state.ui.list.overlay.is_none(), "{leave:?}");
        assert_eq!(state.ui.list.sort, Sort::Attention, "{leave:?}");
    }
}
