use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
};

use crate::{
    app::state::{AppState, Mode},
    providers::github,
    tui::{
        Outcome, pr_detail::{self}, pr_list
    },
};

pub mod actions;
pub mod state;

pub struct App {
    pub state: Arc<Mutex<AppState>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState {
                loading: true,
                ..AppState::default()
            })),
        }
    }

    pub fn load_prs(&self) -> std::io::Result<()> {
        let prs = github::fetch_prs();
        let mut state = self.state.lock().unwrap();
        state.prs = prs;
        state.loading = false;
        Ok(())
    }

    pub fn run(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            let state = self.state.lock().unwrap();
            terminal.draw(|frame| crate::tui::render(frame, &state))?;
            drop(state);

            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let mut state = self.state.lock().unwrap();
                    let outcome = match state.mode {
                        Mode::PrList => pr_list::handle_key(&mut state, key.code),
                        Mode::PrDetail => pr_detail::handle_key(&mut state, key.code),
                    };
                    if matches!(outcome, Outcome::Quit) {
                        return Ok(());
                    }
                }
            }
        }
    }
}
