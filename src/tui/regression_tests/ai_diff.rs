//! The diff's file list counts an agent's comments apart from people's.

use super::support::*;
use crate::domain::{
    authorship::AiMarkers,
    comment::{Comment, CommentThread, ThreadAnchor},
    user::AccountKind,
};

fn comment(id: u64, account: AccountKind, content: &str) -> Comment {
    Comment {
        id: Some(CommentId(id)),
        author: User {
            username: "alice".into(),
        },
        account,
        content: content.into(),
        created: chrono::Utc::now(),
        reactions: vec![],
        reply_to: None,
    }
}

fn thread(comments: Vec<Comment>) -> CommentThread {
    CommentThread {
        comments,
        reply_to: Some(CommentId(1)),
        anchor: Some(ThreadAnchor {
            revision: None,
            path: "src/main.rs".into(),
            line: Some(LineRef::New(1)),
            resolved: false,
            handle: Some(ThreadHandle::NodeId("t".into())),
        }),
    }
}

fn diff_tab(threads: Vec<CommentThread>) -> AppState {
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
        threads,
    });
    state
}

fn screen(state: &mut AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    rendered_text(&terminal)
}

#[test]
fn a_file_shows_its_agents_comments_apart_from_its_peoples() {
    let mut state = diff_tab(vec![
        thread(vec![
            comment(1, AccountKind::Bot, "finding"),
            comment(2, AccountKind::Person, "will do"),
        ]),
        thread(vec![comment(3, AccountKind::Person, "nit")]),
    ]);
    let text = screen(&mut state);
    assert!(text.contains("• 2"), "two comments by people: {text}");
    assert!(text.contains("◆ 1"), "one by the agent: {text}");
}

#[test]
fn the_markers_in_the_config_count_too_and_a_file_without_an_agent_has_no_badge() {
    let mut state = diff_tab(vec![thread(vec![comment(
        1,
        AccountKind::Person,
        "> **gator-agent**\nfinding",
    )])]);
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
