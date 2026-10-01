//! A long comment is folded in the Overview until `space` opens it.

use super::support::*;
use crate::domain::comment::Comment;

fn long_body() -> String {
    (1..=30)
        .map(|n| format!("walkthrough line {n}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn overview_with(contents: &[String]) -> AppState {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    let comments = contents
        .iter()
        .enumerate()
        .map(|(i, content)| Comment {
            id: Some(CommentId(i as u64 + 1)),
            author: User {
                username: "alice".into(),
            },
            account: crate::domain::user::AccountKind::Person,
            content: content.clone(),
            created: chrono::Utc::now(),
            reactions: vec![],
            reply_to: None,
        })
        .collect();
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments,
        events: vec![],
        threads: vec![],
    });
    state
}

fn screen(state: &mut AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 160)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    rendered_text(&terminal)
}

#[test]
fn space_opens_and_folds_a_long_comment() {
    let mut state = overview_with(&[long_body()]);

    let folded = screen(&mut state);
    assert!(folded.contains("walkthrough line 1"), "{folded}");
    assert!(
        !folded.contains("walkthrough line 30"),
        "the tail is folded"
    );
    assert!(folded.contains("more lines · space expand"), "{folded}");

    local_key(&mut state, KeyCode::Char(' '));
    let open = screen(&mut state);
    assert!(open.contains("walkthrough line 1") && !open.contains("more lines"));
    assert_eq!(state.ui.detail.overview.timeline.expanded.len(), 1);

    local_key(&mut state, KeyCode::Char(' '));
    let again = screen(&mut state);
    assert!(again.contains("more lines · space expand"), "folded again");
}

#[test]
fn a_short_comment_is_never_folded_and_space_is_harmless_on_it() {
    let mut state = overview_with(&["Looks right to me.".to_owned()]);
    local_key(&mut state, KeyCode::Char(' '));
    let text = screen(&mut state);
    assert!(text.contains("Looks right to me.") && !text.contains("more lines"));
}

#[test]
fn opening_one_comment_leaves_the_others_folded() {
    let mut state = overview_with(&[long_body(), long_body()]);
    // The cursor is set by drawing, as it always is before a key.
    screen(&mut state);
    local_key(&mut state, KeyCode::Char(' '));
    let text = screen(&mut state);
    assert_eq!(
        text.matches("more lines · space expand").count(),
        1,
        "only the other one is still folded: {text}"
    );
}
