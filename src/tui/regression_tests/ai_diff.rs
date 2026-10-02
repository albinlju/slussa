//! The diff's file list counts an agent's comments apart from people's.

use super::support::*;
use crate::domain::authorship::AiMarkers;

fn diff_tab(threads: Vec<CommentThread>) -> AppState {
    pr_on(
        DetailTab::Diff,
        Activity {
            threads,
            ..Activity::default()
        },
    )
}

#[test]
fn a_file_shows_its_agents_comments_apart_from_its_peoples() {
    let mut state = diff_tab(vec![
        thread(
            1,
            false,
            vec![bot_comment(1, "finding"), comment(2, "will do")],
        ),
        thread(3, false, vec![comment(3, "nit")]),
    ]);
    let text = screen(&mut state);
    assert!(text.contains("• 2"), "two comments by people: {text}");
    assert!(text.contains("◆ 1"), "one by the agent: {text}");
}

#[test]
fn the_markers_in_the_config_count_too_and_a_file_without_an_agent_has_no_badge() {
    let mut state = diff_tab(vec![thread(
        1,
        false,
        vec![comment(1, "> **gator-agent**\nfinding")],
    )]);
    let none = screen(&mut state);
    assert!(!none.contains('◆'), "no marker, no agent: {none}");
    assert!(none.contains("• 1"));

    state.store.ai_markers = AiMarkers::from_config(&["> **gator-agent**".into()]).0;
    let marked = screen(&mut state);
    assert!(marked.contains("◆ 1"), "{marked}");
    assert!(
        !marked.contains("• 1"),
        "nothing is left for people: {marked}"
    );
}
