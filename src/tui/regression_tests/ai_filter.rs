//! The Overview's filter between people's comments and an agent's.

use super::support::*;
use crate::domain::{authorship::AiMarkers, comment::Comment};

fn overview_with(contents: &[&str]) -> AppState {
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
            content: (*content).into(),
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
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    rendered_text(&terminal)
}

#[test]
fn f_cycles_between_all_people_and_the_agent() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    state.store.ai_markers = AiMarkers::from_config(&["> **gator-agent**".into()]).0;

    let all = screen(&mut state);
    assert!(all.contains("Human note") && all.contains("Agent finding"));
    assert!(all.contains("f: comments (all)"), "{all}");

    local_key(&mut state, KeyCode::Char('f'));
    let humans = screen(&mut state);
    assert!(humans.contains("Human note") && !humans.contains("Agent finding"));
    assert!(humans.contains("f: comments (humans)"));

    local_key(&mut state, KeyCode::Char('f'));
    let ai = screen(&mut state);
    assert!(!ai.contains("Human note") && ai.contains("Agent finding"));
    assert!(ai.contains("f: comments (AI)"));

    local_key(&mut state, KeyCode::Char('f'));
    let all = screen(&mut state);
    assert!(all.contains("Human note") && all.contains("Agent finding"));
}

#[test]
fn a_filter_that_leaves_nothing_says_so_and_can_be_changed_again() {
    let mut state = overview_with(&["Human note"]);
    state.store.ai_markers = AiMarkers::from_config(&["> **gator-agent**".into()]).0;
    local_key(&mut state, KeyCode::Char('f'));
    local_key(&mut state, KeyCode::Char('f'));
    let ai = screen(&mut state);
    assert!(ai.contains("(no AI comments)"), "{ai}");
    assert!(ai.contains("f: comments (AI)"));
    local_key(&mut state, KeyCode::Char('f'));
    assert!(screen(&mut state).contains("Human note"));
}

#[test]
fn without_markers_there_is_no_filter_key_and_nothing_is_offered() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    assert!(state.store.ai_markers.is_empty());
    let action = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
    );
    assert!(action.is_none());
    let text = screen(&mut state);
    assert!(text.contains("Agent finding"), "nothing is hidden");
    assert!(!text.contains("f: comments"), "{text}");
}
