//! The mark on a PR that changed since it was opened.

use super::support::*;
use chrono::Duration;

fn list_with(seen_at: Option<Duration>) -> AppState {
    let mut state = fixture();
    state.screen = Screen::List;
    if let Some(offset) = seen_at {
        let LoadState::Loaded(prs) = &state.store.cache.prs else {
            panic!("the fixture's list is loaded");
        };
        let updated = prs[0].updated;
        state
            .store
            .seen
            .mark(prs[0].id, updated + offset, updated + offset);
    }
    state
}

const WIDTH: u16 = 120;

/// The rows of a screen drawn `WIDTH` wide, which `draw` gives as one string.
fn rows(text: &str) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(usize::from(WIDTH))
        .map(|row| row.iter().collect())
        .collect()
}

/// The column where `needle` starts on the first row that has it.
fn column(text: &str, needle: &str) -> Option<usize> {
    rows(text)
        .iter()
        .find_map(|row| row.find(needle).map(|byte| row[..byte].chars().count()))
}

#[test]
fn a_pr_that_changed_since_it_was_opened_has_a_dot_before_its_number() {
    // Opened an hour before the last update: unread.
    let unread = draw(&mut list_with(Some(-Duration::hours(1))), WIDTH, 12);
    assert!(unread.contains("● #42"), "{unread}");

    // Never opened, or opened after the last update: no dot.
    for seen in [None, Some(Duration::zero()), Some(Duration::hours(1))] {
        let text = draw(&mut list_with(seen), WIDTH, 12);
        assert!(
            !text.contains('●') && text.contains("#42"),
            "{seen:?}: {text}"
        );
    }
}

#[test]
fn the_numbers_stay_where_they_are_when_the_dot_comes_and_goes() {
    let with = draw(&mut list_with(Some(-Duration::hours(1))), WIDTH, 12);
    let without = draw(&mut list_with(None), WIDTH, 12);
    assert!(column(&with, "#42").is_some());
    assert_eq!(column(&with, "#42"), column(&without, "#42"));
    // And the heading's `#` stands over the numbers.
    let heading = rows(&without).into_iter().find(|row| row.contains("Title"));
    let heading_hash = heading.and_then(|row| row.find('#').map(|b| row[..b].chars().count()));
    assert_eq!(heading_hash, column(&without, "#42"));
}
