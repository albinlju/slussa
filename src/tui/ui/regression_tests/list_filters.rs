//! The list's search with filters among the words.

use super::support::*;
use crate::domain::review::{Reviewer, ReviewerState};

/// Three PRs: #1 by alice with failing checks, #2 by bob, #3 by alice, approved
/// by a reviewer.
fn list() -> AppState {
    let mut state = fixture();
    state.screen = Screen::List;
    let LoadState::Loaded(prs) = &mut state.store.cache.prs else {
        panic!("the fixture's list is loaded");
    };
    let base = prs[0].clone();
    let by = |id: u64, author: &str, edit: &dyn Fn(&mut PullRequest)| {
        let mut pr = base.clone();
        pr.id = PrId(id);
        pr.title = format!("Change {id}");
        pr.author = User {
            username: author.into(),
        };
        edit(&mut pr);
        pr
    };
    let one = by(1, "alice", &|pr| pr.ci = CiSummary::Failed);
    let two = by(2, "bob", &|_| {});
    let three = by(3, "alice", &|pr| {
        pr.reviewers = vec![Reviewer {
            author: User {
                username: "carol".into(),
            },
            state: ReviewerState::Approved,
        }];
    });
    *prs = vec![one, two, three];
    state
}

/// The rows left after typing `query` into the search field.
fn rows_after(query: &str) -> Vec<u64> {
    let mut state = list();
    local_key(&mut state, KeyCode::Char('/'));
    for typed in query.chars() {
        local_key(&mut state, KeyCode::Char(typed));
    }
    let text = draw(&mut state, 120, 14);
    [1, 2, 3]
        .into_iter()
        .filter(|id| text.contains(&format!("Change {id}")))
        .collect()
}

#[test]
fn filters_narrow_the_list_as_they_are_typed() {
    assert_eq!(rows_after(""), [1, 2, 3]);
    assert_eq!(rows_after("author:alice"), [1, 3]);
    assert_eq!(rows_after("review:none"), [1, 2]);
    assert_eq!(rows_after("ci:failed"), [1]);
    assert_eq!(rows_after("review:approved"), [3]);
    // Together, and with words.
    assert_eq!(rows_after("author:alice ci:failed"), [1]);
    assert_eq!(rows_after("author:alice change 3"), [3]);
}

#[test]
fn a_half_typed_filter_leaves_the_list_as_it_is_until_it_is_one() {
    assert_eq!(rows_after("ci:"), [1, 2, 3]);
    assert_eq!(
        rows_after("review:app"),
        [3],
        "the only value that begins so"
    );
    assert_eq!(
        rows_after("review:x"),
        [1, 2, 3],
        "not a value: nothing filtered"
    );
}

#[test]
fn the_empty_field_says_what_it_understands_and_a_typed_one_does_not() {
    let mut state = list();
    local_key(&mut state, KeyCode::Char('/'));
    let empty = draw(&mut state, 120, 14);
    assert!(empty.contains("author:  review:  ci:"), "{empty}");
    local_key(&mut state, KeyCode::Char('c'));
    let typed = draw(&mut state, 120, 14);
    assert!(
        !typed.contains("review:") && typed.contains("match"),
        "{typed}"
    );
}
