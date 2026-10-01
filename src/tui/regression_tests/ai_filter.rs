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
            account: crate::domain::user::AccountKind::Person,
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
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    state.store.ai_markers = AiMarkers::from_config(&["> **gator-agent**".into()]).0;
    // Humans only, then remove the agent's comment as a refresh could.
    local_key(&mut state, KeyCode::Char('f'));
    if let LoadState::Loaded(activity) = &mut state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity
    {
        activity.comments.remove(1);
    }
    local_key(&mut state, KeyCode::Char('f'));
    let ai = screen(&mut state);
    assert!(ai.contains("(no comments from AI)"), "{ai}");
    assert!(ai.contains("f: comments (AI)"));
    local_key(&mut state, KeyCode::Char('f'));
    let all = screen(&mut state);
    assert!(all.contains("Human note"));
    assert!(
        !all.contains("f: comments"),
        "nothing to filter on any more"
    );
}

#[test]
fn without_markers_there_is_no_filter_key_and_nothing_is_offered() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    assert_eq!(state.store.ai_markers, AiMarkers::default());
    let action = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
    );
    assert!(action.is_none());
    let text = screen(&mut state);
    assert!(text.contains("Agent finding"), "nothing is hidden");
    assert!(!text.contains("f: comments"), "{text}");
}

#[test]
fn a_bots_account_offers_the_filter_without_any_configuration() {
    let mut state = overview_with(&["Human note", "Agent finding"]);
    assert_eq!(state.store.ai_markers, AiMarkers::default());
    if let LoadState::Loaded(activity) = &mut state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity
    {
        activity.comments[1].account = crate::domain::user::AccountKind::Bot;
    }
    assert!(screen(&mut state).contains("f: comments (all)"));
    local_key(&mut state, KeyCode::Char('f'));
    let humans = screen(&mut state);
    assert!(humans.contains("Human note") && !humans.contains("Agent finding"));
    local_key(&mut state, KeyCode::Char('f'));
    let ai = screen(&mut state);
    assert!(!ai.contains("Human note") && ai.contains("Agent finding"));
}

#[test]
fn a_filter_that_is_on_stays_offered_when_the_agents_comments_are_gone() {
    let mut state = overview_with(&["Human note"]);
    state.store.ai_markers = AiMarkers::from_config(&["> **gator-agent**".into()]).0;
    // Markers alone no longer offer the key: there is nothing from an agent.
    assert!(!screen(&mut state).contains("f: comments"));
    // A filter that was turned on while the agent's comment was there must
    // still be possible to turn off after a refresh took it away.
    state.ui.detail.overview.timeline.filter = crate::domain::authorship::AuthorFilter::Ai;
    let text = screen(&mut state);
    assert!(text.contains("f: comments (AI)"), "{text}");
    local_key(&mut state, KeyCode::Char('f'));
    let all = screen(&mut state);
    assert!(all.contains("Human note"));
    assert!(
        !all.contains("f: comments"),
        "nothing to filter on any more"
    );
}
