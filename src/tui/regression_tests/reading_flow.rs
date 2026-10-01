//! `j` and `k` in the Overview read on through a comment taller than the screen
//! before they move to the next one, so none of it is skipped.

use super::support::*;
use crate::domain::{comment::Comment, user::AccountKind};

const LAST: &str = "walkthrough line 60";

fn comment(id: u64, content: String, age_minutes: i64) -> Comment {
    Comment {
        id: Some(CommentId(id)),
        author: User {
            username: "alice".into(),
        },
        account: AccountKind::Person,
        content,
        created: chrono::Utc::now() - chrono::Duration::minutes(age_minutes),
        reactions: vec![],
        reply_to: None,
    }
}

/// A tall comment, opened, above a short older one: the timeline is newest first.
fn tall_then_short() -> AppState {
    let tall = (1..=60)
        .map(|n| format!("walkthrough line {n}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![comment(1, tall, 0), comment(2, "A short reply.".into(), 60)],
        events: vec![],
        threads: vec![],
    });
    screen(&mut state);
    local_key(&mut state, KeyCode::Char(' '));
    screen(&mut state);
    state
}

fn screen(state: &mut AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    rendered_text(&terminal)
}

/// A key, then a draw, as in the event loop.
fn press(state: &mut AppState, code: KeyCode) -> String {
    local_key(state, code);
    screen(state)
}

fn cursor(state: &AppState) -> usize {
    state.ui.detail.overview.timeline.cursor
}

#[test]
fn j_reads_through_a_tall_comment_before_it_moves_to_the_next() {
    let mut state = tall_then_short();
    assert_eq!(cursor(&state), 0);
    assert!(
        !screen(&mut state).contains(LAST),
        "the end is below the screen"
    );

    let mut seen_the_end = false;
    let mut last_scroll = 0;
    for _ in 0..80 {
        let text = press(&mut state, KeyCode::Char('j'));
        seen_the_end |= text.contains(LAST);
        let scroll = state.ui.detail.overview.timeline.scroll;
        assert!(scroll >= last_scroll, "j never scrolls back");
        last_scroll = scroll;
        if cursor(&state) == 1 {
            break;
        }
    }
    assert!(
        seen_the_end,
        "the cursor left the comment before its end was shown"
    );
    assert_eq!(
        cursor(&state),
        1,
        "and it did move on once the end was shown"
    );
    assert!(screen(&mut state).contains("A short reply."));
}

#[test]
fn j_moves_at_once_between_items_that_fit_on_the_screen() {
    let mut state = tall_then_short();
    // Fold the tall comment again: both items fit, so one press moves on.
    local_key(&mut state, KeyCode::Char(' '));
    screen(&mut state);
    press(&mut state, KeyCode::Char('j'));
    assert_eq!(cursor(&state), 1);
}

#[test]
fn k_steps_back_up_into_a_tall_comment_at_its_end_and_reads_upward() {
    let mut state = tall_then_short();
    for _ in 0..80 {
        press(&mut state, KeyCode::Char('j'));
        if cursor(&state) == 1 {
            break;
        }
    }
    assert_eq!(cursor(&state), 1);

    // Back into the tall one: its end is what is shown, not its first line.
    let text = press(&mut state, KeyCode::Char('k'));
    assert_eq!(cursor(&state), 0);
    assert!(text.contains(LAST), "{text}");
    assert!(!text.contains("walkthrough line 1 ") && !text.contains("walkthrough line 1\u{a0}"));

    // Reading upward: k scrolls up through it, and stays on it until its start shows.
    let mut seen_the_start = false;
    for _ in 0..80 {
        let text = press(&mut state, KeyCode::Char('k'));
        seen_the_start |=
            text.contains("walkthrough line 1 ") || text.contains("walkthrough line 1│");
        assert_eq!(cursor(&state), 0);
        if seen_the_start {
            break;
        }
    }
    assert!(seen_the_start, "k reached the start of the comment");
}
