//! A long comment in the diff's inline threads is folded until `space` on its
//! fold row opens it.

use super::support::*;
use crate::domain::{
    comment::{Comment, CommentThread, ThreadAnchor},
    user::AccountKind,
};

fn body(lines: usize) -> String {
    (1..=lines)
        .map(|n| format!("finding line {n}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn comment(id: u64, content: String) -> Comment {
    Comment {
        id: Some(CommentId(id)),
        author: User {
            username: "alice".into(),
        },
        account: AccountKind::Person,
        content,
        created: chrono::Utc::now(),
        reactions: vec![],
        reply_to: None,
    }
}

fn thread(root: u64, content: String) -> CommentThread {
    thread_resolved(root, content, false)
}

fn thread_resolved(root: u64, content: String, resolved: bool) -> CommentThread {
    CommentThread {
        comments: vec![comment(root, content)],
        reply_to: Some(CommentId(root)),
        anchor: Some(ThreadAnchor {
            revision: None,
            path: "src/main.rs".into(),
            line: Some(LineRef::New(1)),
            resolved,
            handle: Some(ThreadHandle::NodeId("t".into())),
        }),
    }
}

fn diff_with(content: String) -> AppState {
    diff_with_thread(thread(1, content))
}

fn diff_with_thread(thread: CommentThread) -> AppState {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Diff,
    };
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![],
        events: vec![],
        threads: vec![thread],
    });
    state
}

fn screen(state: &mut AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 80)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    rendered_text(&terminal)
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
        if footer(&screen(state)).contains(footer_has) {
            return;
        }
        local_key(state, KeyCode::Char('j'));
    }
    panic!("the cursor never got to a stop where the footer says {footer_has:?}");
}

#[test]
fn a_long_comment_in_the_diff_has_a_fold_row_the_cursor_can_stop_on() {
    let mut state = diff_with(body(30));
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
        footer(&open).contains("space: fold comment"),
        "{}",
        footer(&open)
    );

    // The cursor is still on that row, so space folds it again.
    local_key(&mut state, KeyCode::Char(' '));
    let folded = screen(&mut state);
    assert!(folded.contains("more lines · space expand") && !folded.contains("finding line 30"));
}

#[test]
fn a_reply_still_goes_to_the_thread_from_its_fold_row() {
    let mut state = diff_with(body(30));
    into_the_code(&mut state);
    down_until(&mut state, "space: expand comment");
    assert!(footer(&screen(&mut state)).contains("r: reply"));
}

#[test]
fn a_short_comment_in_the_diff_is_left_whole() {
    let mut state = diff_with(body(3));
    into_the_code(&mut state);
    let text = screen(&mut state);
    assert!(
        text.contains("finding line 3") && !text.contains("more lines"),
        "{text}"
    );
}

fn footer(text: &str) -> String {
    text.chars()
        .rev()
        .take(140)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

#[test]
fn a_resolved_thread_opens_and_closes_with_one_space_and_its_fold_row_is_separate() {
    let mut state = diff_with_thread(thread_resolved(1, body(30), true));
    into_the_code(&mut state);
    let shut = screen(&mut state);
    assert!(!shut.contains("finding line 1"), "{shut}");
    assert!(
        footer(&shut).contains("space: expand thread"),
        "{}",
        footer(&shut)
    );

    // One space opens the thread, and its long comment is still folded.
    local_key(&mut state, KeyCode::Char(' '));
    let folded = screen(&mut state);
    assert!(
        folded.contains("finding line 1") && !folded.contains("finding line 30"),
        "{folded}"
    );
    assert!(
        footer(&folded).contains("space: collapse thread"),
        "{}",
        footer(&folded)
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
    let mut state = diff_with_thread(thread_resolved(1, body(2), true));
    into_the_code(&mut state);
    assert!(!screen(&mut state).contains("finding line 1"));
    local_key(&mut state, KeyCode::Char(' '));
    assert!(screen(&mut state).contains("finding line 2"));
    local_key(&mut state, KeyCode::Char(' '));
    assert!(!screen(&mut state).contains("finding line 1"));
}
