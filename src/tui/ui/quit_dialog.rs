//! The question asked when the reader quits while an agent is reviewing a PR, and
//! the wait for it to stop. The keys are the application's (`app::quit`).

use crate::tui::{
    app::{
        quit::{QuitAnswer, QuitChoice, QuitGate},
        state::AppState,
        store::{FetchKey, PrResource},
    },
    ui::{theme, widgets},
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// What a key means to the question about leaving.
pub const fn answer(key: KeyEvent) -> QuitAnswer {
    match key.code {
        KeyCode::Left
        | KeyCode::Right
        | KeyCode::Up
        | KeyCode::Down
        | KeyCode::Char('h' | 'j' | 'k' | 'l') => QuitAnswer::Switch,
        KeyCode::Enter => QuitAnswer::Take,
        KeyCode::Char('y') => QuitAnswer::Yes,
        KeyCode::Esc | KeyCode::Char('n') => QuitAnswer::No,
        _ => QuitAnswer::Nothing,
    }
}

pub(super) fn render(frame: &mut Frame<'_>, state: &AppState) {
    let area = frame.area();
    match state.store.quit {
        QuitGate::Open => {}
        QuitGate::Asking(choice) => ask(frame, area, &running_reviews(state), choice),
        QuitGate::Waiting { .. } => wait(frame, area),
    }
}

/// The PRs an agent is reviewing now, in order.
fn running_reviews(state: &AppState) -> Vec<u64> {
    let mut ids: Vec<u64> = state
        .store
        .fetches
        .iter()
        .filter_map(|key| match key {
            FetchKey::Pr(PrResource::AgentReview, id) => Some(id.0),
            FetchKey::Pr(..) | FetchKey::Prs(_) | FetchKey::One(_) => None,
        })
        .collect();
    ids.sort_unstable();
    ids
}

fn ask(frame: &mut Frame<'_>, area: ratatui::layout::Rect, reviews: &[u64], choice: QuitChoice) {
    let theme = theme::current();
    let numbers: Vec<String> = reviews.iter().map(|id| format!("#{id}")).collect();
    let what = match reviews {
        [] => "An agent is reviewing a PR.".to_owned(),
        [_] => format!("An agent is reviewing PR {}.", numbers.join("")),
        _ => format!("Agents are reviewing PRs {}.", numbers.join(", ")),
    };
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);
    let answer = |label: &str, is: bool| {
        Line::from(vec![
            Span::styled(
                if is { "▶ " } else { "  " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(format!(" {label} "), if is { selected } else { normal }),
        ])
    };
    let lines = vec![
        Line::styled(what, Style::default().fg(theme.fg)),
        Line::styled(
            "Quitting stops it, and what it has found so far is lost.",
            normal,
        ),
        Line::default(),
        answer(
            "Stop the review and quit",
            choice == QuitChoice::StopAndQuit,
        ),
        answer("Keep it running", choice == QuitChoice::KeepRunning),
    ];
    let inner = widgets::dialog::frame(
        frame,
        area,
        "Quit slussa?",
        (60, 5),
        &[("j/k", "move"), ("Enter", "select"), ("Esc", "cancel")],
    );
    frame.render_widget(Paragraph::new(lines), inner);
}

fn wait(frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let theme = theme::current();
    let lines = vec![
        widgets::loading("Waiting for the agent to stop before slussa closes…"),
        Line::default(),
        Line::styled(
            "It is ended in a moment, and slussa closes when it is.",
            Style::default().fg(theme.muted),
        ),
    ];
    let inner = widgets::dialog::frame(frame, area, "Quitting", (60, 3), &[]);
    frame.render_widget(Paragraph::new(lines), inner);
}
