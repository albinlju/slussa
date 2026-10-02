//! The Overview's filter between people's comments and an agent's.

use super::support::*;
use crate::domain::authorship::AiMarkers;

#[test]
fn f_cycles_between_all_people_and_the_agent() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    state
        .store
        .set_ai_markers(AiMarkers::from_config(&["> **gator-agent**".into()]).0);

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
fn what_the_filter_hides_stays_as_a_dimmed_line_and_f_brings_it_back() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    state
        .store
        .set_ai_markers(AiMarkers::from_config(&["> **gator-agent**".into()]).0);
    // People only: the agent's comment is a line, not a gap.
    local_key(&mut state, KeyCode::Char('f'));
    let humans = screen(&mut state);
    assert!(humans.contains("Human note") && !humans.contains("Agent finding"));
    assert!(
        humans.contains("◆ 1 AI comment hidden · f to cycle"),
        "{humans}"
    );
    // AI only: now the person's comment is the one hidden.
    local_key(&mut state, KeyCode::Char('f'));
    let ai = screen(&mut state);
    assert!(ai.contains("Agent finding") && !ai.contains("Human note"));
    assert!(ai.contains("◆ 1 human comment hidden · f to cycle"), "{ai}");
    // All: nothing is hidden, so there is no line.
    local_key(&mut state, KeyCode::Char('f'));
    let all = screen(&mut state);
    assert!(all.contains("Human note") && all.contains("Agent finding"));
    assert!(!all.contains("hidden"), "{all}");
}

#[test]
fn a_filter_that_hides_everything_leaves_the_line_and_the_way_back() {
    let mut state = overview_with(&["Human note"]);
    state
        .store
        .set_ai_markers(AiMarkers::from_config(&["> **gator-agent**".into()]).0);
    state.ui.detail.overview.timeline.filter = crate::domain::authorship::AuthorFilter::Ai;
    let text = screen(&mut state);
    assert!(text.contains("◆ 1 human comment hidden"), "{text}");
    assert!(text.contains("f: comments (AI)"), "{text}");
    // The cursor has nothing to land on, and j must not panic or move.
    local_key(&mut state, KeyCode::Char('j'));
    assert!(screen(&mut state).contains("hidden"));
}

#[test]
fn without_markers_there_is_no_filter_key_and_nothing_is_offered() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
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
    let mut state = pr_on(
        DetailTab::Overview,
        Activity {
            comments: vec![comment(1, "Human note"), bot_comment(2, "Agent finding")],
            ..Activity::default()
        },
    );
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
    state
        .store
        .set_ai_markers(AiMarkers::from_config(&["> **gator-agent**".into()]).0);
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

#[test]
fn an_agents_comment_carries_an_ai_tag_and_a_persons_does_not() {
    let mut state = overview_with(&["Human note", "> **gator-agent**\nAgent finding"]);
    let plain = screen(&mut state);
    assert!(
        !plain.contains("[AI]"),
        "nothing is marked without a marker"
    );

    state
        .store
        .set_ai_markers(AiMarkers::from_config(&["> **gator-agent**".into()]).0);
    let text: Vec<char> = screen(&mut state).chars().collect();
    // The screen is 140 cells wide, with no line breaks in the text.
    let rows: Vec<String> = text.chunks(140).map(|row| row.iter().collect()).collect();
    let tagged: Vec<&String> = rows.iter().filter(|row| row.contains("[AI]")).collect();
    assert_eq!(tagged.len(), 1, "{rows:#?}");
    assert!(tagged[0].contains("alice [AI]"), "the tag follows the name");
    assert!(
        !rows
            .iter()
            .any(|row| row.contains("Human note") && row.contains("[AI]"))
    );
}
