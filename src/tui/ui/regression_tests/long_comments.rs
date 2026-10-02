//! A long comment is folded in the Overview until `space` opens it.

use super::support::*;

fn long_body() -> String {
    (1..=30)
        .map(|n| format!("walkthrough line {n}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[test]
fn space_opens_and_folds_a_long_comment() {
    let mut state = overview_with(&[&long_body()]);

    let folded = draw(&mut state, 140, 160);
    assert!(folded.contains("walkthrough line 1"), "{folded}");
    assert!(
        !folded.contains("walkthrough line 30"),
        "the tail is folded"
    );
    assert!(folded.contains("more lines · space expand"), "{folded}");

    local_key(&mut state, KeyCode::Char(' '));
    let open = draw(&mut state, 140, 160);
    assert!(open.contains("walkthrough line 1") && !open.contains("more lines"));
    assert_eq!(state.ui.detail.overview.timeline.expanded.len(), 1);

    local_key(&mut state, KeyCode::Char(' '));
    let again = draw(&mut state, 140, 160);
    assert!(again.contains("more lines · space expand"), "folded again");
}

#[test]
fn a_short_comment_is_never_folded_and_space_is_harmless_on_it() {
    let mut state = overview_with(&["Looks right to me."]);
    local_key(&mut state, KeyCode::Char(' '));
    let text = draw(&mut state, 140, 160);
    assert!(text.contains("Looks right to me.") && !text.contains("more lines"));
}

#[test]
fn opening_one_comment_leaves_the_others_folded() {
    let mut state = overview_with(&[&long_body(), &long_body()]);
    // The cursor is set by drawing, as it always is before a key.
    draw(&mut state, 140, 160);
    local_key(&mut state, KeyCode::Char(' '));
    let text = draw(&mut state, 140, 160);
    assert_eq!(
        text.matches("more lines · space expand").count(),
        1,
        "only the other one is still folded: {text}"
    );
}
