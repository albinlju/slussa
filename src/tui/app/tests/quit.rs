//! `q` while an agent is reviewing a PR asks first, and waits for it to stop.

use super::support::*;
use crate::{
    agent::{AgentError, Cancel},
    tui::app::{
        agent_review::Failure,
        quit::{QuitChoice, QuitGate},
        store::{FetchKey, PrResource},
    },
};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Duration;

fn screen_text(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
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

fn key(app: &mut App, code: KeyCode) -> Next {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// An agent is reviewing PR 42, as it is once `A` has been answered.
fn with_a_review_running(app: &mut App) -> Cancel {
    let cancel = Cancel::default();
    app.agent_reviews.insert(PrId(42), cancel.clone());
    app.state
        .store
        .fetches
        .insert(FetchKey::Pr(PrResource::AgentReview, PrId(42)));
    cancel
}

fn stopped(cancel: &Cancel) -> bool {
    format!("{cancel:?}").contains("true")
}

#[test]
fn with_no_review_running_q_quits_at_once() {
    let mut app = app();
    assert_eq!(key(&mut app, KeyCode::Char('q')), Next::Quit);
    detail(&mut app, DetailTab::Overview);
    assert_eq!(key(&mut app, KeyCode::Char('q')), Next::Quit);
}

#[test]
fn with_a_review_running_q_asks_and_the_first_answer_is_to_keep_it_running() {
    let mut app = app();
    let cancel = with_a_review_running(&mut app);
    assert_eq!(key(&mut app, KeyCode::Char('q')), Next::Continue);
    assert_eq!(
        app.state.store.quit,
        QuitGate::Asking(QuitChoice::KeepRunning)
    );
    let text = screen_text(&mut app);
    assert!(text.contains("Quit slussa?"), "{text}");
    assert!(text.contains("An agent is reviewing PR #42."), "{text}");
    assert!(text.contains("what it has found so far is lost"));
    assert!(text.contains("Stop the review and quit") && text.contains("Keep it running"));

    // `q` again is not an answer: asking twice is how a quit is made by mistake.
    assert_eq!(key(&mut app, KeyCode::Char('q')), Next::Continue);
    assert!(matches!(app.state.store.quit, QuitGate::Asking(_)));
    // Enter on what was chosen from the start: it keeps running.
    assert_eq!(key(&mut app, KeyCode::Enter), Next::Continue);
    assert_eq!(app.state.store.quit, QuitGate::Open);
    assert!(!stopped(&cancel));
    assert!(!screen_text(&mut app).contains("Quit slussa?"));
}

#[test]
fn esc_and_n_leave_the_question_without_stopping_anything() {
    for answer in [KeyCode::Esc, KeyCode::Char('n')] {
        let mut app = app();
        let cancel = with_a_review_running(&mut app);
        key(&mut app, KeyCode::Char('q'));
        key(&mut app, KeyCode::Left);
        assert_eq!(key(&mut app, answer), Next::Continue);
        assert_eq!(app.state.store.quit, QuitGate::Open);
        assert!(!stopped(&cancel), "{answer:?}");
    }
}

#[test]
fn choosing_to_stop_waits_with_a_spinner_for_the_agent_and_then_quits() {
    let mut app = app();
    let cancel = with_a_review_running(&mut app);
    key(&mut app, KeyCode::Char('q'));
    key(&mut app, KeyCode::Left);
    assert_eq!(
        app.state.store.quit,
        QuitGate::Asking(QuitChoice::StopAndQuit)
    );
    assert_eq!(key(&mut app, KeyCode::Enter), Next::Continue);
    assert!(matches!(app.state.store.quit, QuitGate::Waiting { .. }));
    assert!(stopped(&cancel), "the review was told to stop");

    let text = screen_text(&mut app);
    assert!(
        text.contains("Waiting for the agent to stop before slussa closes"),
        "{text}"
    );
    assert!(
        "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().any(|glyph| text.contains(glyph)),
        "a spinner"
    );

    // Keys are not for the screen while it waits, and nothing closes yet.
    assert_eq!(key(&mut app, KeyCode::Char('q')), Next::Continue);
    assert!(matches!(app.state.store.quit, QuitGate::Waiting { .. }));
    assert!(!app.waiting_is_over(), "the agent has not reported");

    // The agent has stopped and said so: now slussa closes.
    app.agent_review_done(
        PrId(42),
        Err(Failure::Agent(AgentError::Cancelled {
            program: "claude".into(),
        })),
    );
    assert!(app.waiting_is_over());
}

#[test]
fn y_is_a_yes_and_a_command_that_does_not_die_is_not_waited_for_for_ever() {
    let mut app = app();
    let cancel = with_a_review_running(&mut app);
    key(&mut app, KeyCode::Char('q'));
    key(&mut app, KeyCode::Char('y'));
    assert!(stopped(&cancel));
    assert!(
        !app.waiting_is_over(),
        "not yet: it has only just been asked"
    );
    app.waited_for(Duration::from_secs(6));
    assert!(app.waiting_is_over(), "the wait is over");
}

#[test]
fn the_question_is_asked_from_the_list_too() {
    let mut app = app();
    with_a_review_running(&mut app);
    assert!(matches!(app.state.screen, Screen::List));
    assert_eq!(key(&mut app, KeyCode::Char('q')), Next::Continue);
    assert!(matches!(app.state.store.quit, QuitGate::Asking(_)));
}

#[test]
fn several_reviews_are_named_and_all_are_stopped() {
    let mut app = app();
    let first = with_a_review_running(&mut app);
    let second = Cancel::default();
    app.agent_reviews.insert(PrId(51), second.clone());
    app.state
        .store
        .fetches
        .insert(FetchKey::Pr(PrResource::AgentReview, PrId(51)));
    key(&mut app, KeyCode::Char('q'));
    assert!(screen_text(&mut app).contains("Agents are reviewing PRs #42, #51."));
    key(&mut app, KeyCode::Char('y'));
    assert!(stopped(&first) && stopped(&second));
}
