//! A long comment in the diff's inline threads is folded until `space` on its
//! fold row opens it.

use super::support::*;

fn body(lines: usize) -> String {
    (1..=lines)
        .map(|n| format!("finding line {n}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn diff_with(content: &str) -> AppState {
    diff_with_thread(thread(1, false, vec![comment(1, content)]))
}

fn diff_with_thread(thread: CommentThread) -> AppState {
    pr_on(
        DetailTab::Diff,
        Activity {
            threads: vec![thread],
            ..Activity::default()
        },
    )
}

/// The screen tall enough for a whole long comment.
fn screen(state: &mut AppState) -> String {
    draw(state, 140, 80)
}

/// From the tree's first folder to the file, into the code pane, and onto the
/// thread (the first item after the line it is on).
fn into_the_code(state: &mut AppState) {
    local_key(state, KeyCode::Char('3'));
    screen(state);
    local_key(state, KeyCode::Char('j'));
    local_key(state, KeyCode::Enter);
    // The thread hangs under its line: step down until the reply key is offered.
    for _ in 0..6 {
        if screen(state).contains("r: reply") {
            return;
        }
        local_key(state, KeyCode::Char('j'));
    }
    panic!("the cursor never reached the thread");
}

/// Down to the next stop, until the footer says what `space` does there.
fn down_until(state: &mut AppState, footer_has: &str) {
    for _ in 0..4 {
        if footer_of(&screen(state)).contains(footer_has) {
            return;
        }
        local_key(state, KeyCode::Char('j'));
    }
    panic!("the cursor never got to a stop where the footer says {footer_has:?}");
}

#[test]
fn a_long_comment_in_the_diff_has_a_fold_row_the_cursor_can_stop_on() {
    let mut state = diff_with(&body(30));
    into_the_code(&mut state);
    let folded = screen(&mut state);
    assert!(
        folded.contains("finding line 1") && !folded.contains("finding line 30"),
        "{folded}"
    );
    assert!(folded.contains("more lines · space expand"), "{folded}");
    // On the thread, space does nothing to a comment that is not resolved.
    local_key(&mut state, KeyCode::Char(' '));
    assert!(screen(&mut state).contains("more lines · space expand"));

    // One step down is the fold row itself, and space there opens the comment.
    down_until(&mut state, "space: expand comment");
    local_key(&mut state, KeyCode::Char(' '));
    let open = screen(&mut state);
    assert!(
        open.contains("finding line 30") && !open.contains("more lines"),
        "{open}"
    );
    assert!(open.contains("▲ fold · space"), "{open}");
    assert!(
        footer_of(&open).contains("space: fold comment"),
        "{}",
        footer_of(&open)
    );

    // The cursor is still on that row, so space folds it again.
    local_key(&mut state, KeyCode::Char(' '));
    let folded = screen(&mut state);
    assert!(folded.contains("more lines · space expand") && !folded.contains("finding line 30"));
}

#[test]
fn a_reply_still_goes_to_the_thread_from_its_fold_row() {
    let mut state = diff_with(&body(30));
    into_the_code(&mut state);
    down_until(&mut state, "space: expand comment");
    assert!(footer_of(&screen(&mut state)).contains("r: reply"));
}

#[test]
fn a_short_comment_in_the_diff_is_left_whole() {
    let mut state = diff_with(&body(3));
    into_the_code(&mut state);
    let text = screen(&mut state);
    assert!(
        text.contains("finding line 3") && !text.contains("more lines"),
        "{text}"
    );
}

#[test]
fn a_resolved_thread_opens_and_closes_with_one_space_and_its_fold_row_is_separate() {
    let mut state = diff_with_thread(thread(1, true, vec![comment(1, &body(30))]));
    into_the_code(&mut state);
    let shut = screen(&mut state);
    assert!(!shut.contains("finding line 1"), "{shut}");
    assert!(
        footer_of(&shut).contains("space: expand thread"),
        "{}",
        footer_of(&shut)
    );

    // One space opens the thread, and its long comment is still folded.
    local_key(&mut state, KeyCode::Char(' '));
    let folded = screen(&mut state);
    assert!(
        folded.contains("finding line 1") && !folded.contains("finding line 30"),
        "{folded}"
    );
    assert!(
        footer_of(&folded).contains("space: collapse thread"),
        "{}",
        footer_of(&folded)
    );

    // One space closes it again, whatever the comment inside has done.
    local_key(&mut state, KeyCode::Char(' '));
    assert!(!screen(&mut state).contains("finding line 1"));

    // Open it, step to the fold row, and open the comment there.
    local_key(&mut state, KeyCode::Char(' '));
    down_until(&mut state, "space: expand comment");
    local_key(&mut state, KeyCode::Char(' '));
    let whole = screen(&mut state);
    assert!(
        whole.contains("finding line 30") && !whole.contains("more lines"),
        "{whole}"
    );
}

#[test]
fn a_short_resolved_thread_opens_and_closes_with_one_space_each() {
    let mut state = diff_with_thread(thread(1, true, vec![comment(1, &body(2))]));
    into_the_code(&mut state);
    assert!(!screen(&mut state).contains("finding line 1"));
    local_key(&mut state, KeyCode::Char(' '));
    assert!(screen(&mut state).contains("finding line 2"));
    local_key(&mut state, KeyCode::Char(' '));
    assert!(!screen(&mut state).contains("finding line 1"));
}
