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

                    match state.mode {
                        Mode::PrList => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Down | KeyCode::Char('j') => {
                                if state.selected_pr + 1 < state.prs.len() {
                                    state.selected_pr += 1;
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if state.selected_pr > 0 {
                                    state.selected_pr -= 1;
                                }
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }
}
