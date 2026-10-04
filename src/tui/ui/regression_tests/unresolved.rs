//! `u` and `U` in the Overview: from one unresolved review thread to the next.

use super::support::*;
use chrono::Duration;

/// A review thread with root comment `root`, started `minutes` after the
/// others' base time, so that the timeline (newest first) has a known order.
fn thread_at(root: u64, resolved: bool, minutes: i64) -> CommentThread {
    let mut thread = thread(root, resolved, vec![comment(root, "A finding")]);
    for comment in &mut thread.comments {
        comment.created += Duration::minutes(minutes);
    }
    thread
}

/// Newest first: 30 (not resolved), 20 (resolved), 10 (not resolved).
fn overview() -> AppState {
    pr_on(
        DetailTab::Overview,
        Activity {
            threads: vec![
                thread_at(10, false, 0),
                thread_at(20, true, 1),
                thread_at(30, false, 2),
            ],
            ..Activity::default()
        },
    )
}

/// The root comment of the thread the cursor is on, after a draw.
fn focused(state: &mut AppState) -> Option<u64> {
    draw(state, 140, 30);
    state
        .ui
        .detail
        .overview
        .timeline
        .thread
        .as_ref()
        .and_then(|thread| thread.comment_id)
        .map(|id| id.0)
}

#[test]
fn u_goes_between_the_unresolved_threads_and_skips_the_resolved_one() {
    let mut state = overview();
    assert_eq!(focused(&mut state), Some(30), "the newest is first");

    local_key(&mut state, KeyCode::Char('u'));
    assert_eq!(focused(&mut state), Some(10), "past 20, which is resolved");

    local_key(&mut state, KeyCode::Char('u'));
    assert_eq!(
        focused(&mut state),
        Some(30),
        "round from the last to the first"
    );
}

#[test]
fn capital_u_goes_the_other_way_round() {
    let mut state = overview();
    assert_eq!(focused(&mut state), Some(30));
    local_key(&mut state, KeyCode::Char('U'));
    assert_eq!(
        focused(&mut state),
        Some(10),
        "round from the first to the last"
    );
    local_key(&mut state, KeyCode::Char('U'));
    assert_eq!(focused(&mut state), Some(30), "past 20, which is resolved");
}

#[test]
fn with_every_thread_resolved_u_does_nothing_and_the_footer_does_not_mention_it() {
    let mut state = pr_on(
        DetailTab::Overview,
        Activity {
            threads: vec![thread_at(10, true, 0), thread_at(20, true, 1)],
            ..Activity::default()
        },
    );
    let before = focused(&mut state);
    local_key(&mut state, KeyCode::Char('u'));
    assert_eq!(focused(&mut state), before);
    assert!(!draw(&mut state, 200, 30).contains("u: unresolved"));
}

#[test]
fn the_footer_says_how_many_are_left_when_there_is_room_for_it() {
    let mut state = overview();
    let text = draw(&mut state, 200, 30);
    assert!(text.contains("u: unresolved (2)"), "{text}");
}
