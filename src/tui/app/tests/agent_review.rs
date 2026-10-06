//! `A` asks first, names what it runs, and is off where it makes no sense.

use super::support::*;
use crate::{
    domain::pr::PrStatus,
    test_support::TempDir,
    tui::app::{
        proposals::ProposalsSource,
        store::{FetchKey, PrResource},
    },
};
use ratatui::{Terminal, backend::TestBackend};

fn screen_text(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &mut app.state))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn app_with_agent(dir: &TempDir) -> App {
    let mut app = app();
    app.state.store.agent_review = vec!["claude".into(), "-p".into()];
    app.proposals_source = Some(ProposalsSource {
        root: dir.path().to_path_buf(),
        scope: "scope".into(),
    });
    detail(&mut app, DetailTab::Overview);
    app
}

fn running(app: &App) -> bool {
    app.state
        .store
        .fetches
        .contains(&FetchKey::Pr(PrResource::AgentReview, PrId(42)))
}

#[tokio::test(flavor = "current_thread")]
async fn a_asks_what_it_will_run_and_enter_runs_it() {
    let dir = TempDir::new("agent-key");
    let mut app = app_with_agent(&dir);
    assert!(screen_text(&mut app).contains("A: agent review"));

    press(&mut app, KeyCode::Char('A'));
    let text = screen_text(&mut app);
    assert!(text.contains("Ask an agent to review this PR?"), "{text}");
    assert!(text.contains("Runs: claude -p"), "it names the command");
    assert!(
        text.contains("title, description and diff"),
        "and what it is given"
    );
    assert!(text.contains("nothing is posted"));
    assert!(!running(&app), "nothing has started yet");

    press(&mut app, KeyCode::Enter);
    assert!(running(&app));
    let text = screen_text(&mut app);
    assert!(
        text.contains("A: agent review (running)"),
        "a second one is not offered while it runs"
    );
    // A spinner that moves, and what is going on, so the wait is seen to be one.
    assert!(text.contains("agent reviewing…"), "{text}");
    assert!(
        "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
            .chars()
            .any(|glyph| text.contains(&format!("{glyph} agent reviewing"))),
        "a spinner beside it"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn esc_in_the_question_runs_nothing() {
    let dir = TempDir::new("agent-key");
    let mut app = app_with_agent(&dir);
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Esc);
    assert!(!running(&app));
    assert!(!screen_text(&mut app).contains("Ask an agent to review"));
}

#[tokio::test(flavor = "current_thread")]
async fn without_a_command_the_key_is_not_there() {
    let dir = TempDir::new("agent-key");
    let mut app = app_with_agent(&dir);
    app.state.store.agent_review = Vec::new();
    assert!(!screen_text(&mut app).contains("A: agent review"));
    press(&mut app, KeyCode::Char('A'));
    assert!(!screen_text(&mut app).contains("Ask an agent"));
}

#[tokio::test(flavor = "current_thread")]
async fn a_pr_that_is_merged_says_why_the_key_does_nothing() {
    let dir = TempDir::new("agent-key");
    let mut app = app_with_agent(&dir);
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].status = PrStatus::Merged;
    }
    assert!(screen_text(&mut app).contains("A: agent review (merged)"));
    press(&mut app, KeyCode::Char('A'));
    assert!(!screen_text(&mut app).contains("Ask an agent"));
}

#[tokio::test(flavor = "current_thread")]
async fn a_provider_that_cannot_give_the_diff_as_text_has_no_key() {
    let dir = TempDir::new("agent-key");
    let mut app = app_with_agent(&dir);
    app.state
        .store
        .capabilities
        .features
        .remove(&crate::domain::capabilities::Feature::AgentReview);
    assert!(!screen_text(&mut app).contains("A: agent review"));
    press(&mut app, KeyCode::Char('A'));
    assert!(!screen_text(&mut app).contains("Ask an agent"));
}
