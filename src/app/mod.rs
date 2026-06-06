use std::time::Duration;

use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, EventStream, KeyEventKind},
};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time,
};
use tokio_stream::StreamExt;

use crate::{
    app::state::{AppState, LoadState},
    tui::{Action, key_to_action, render},
};

pub mod fetchers;
pub mod reducer;
pub mod state;

pub struct App {
    pub state: AppState,
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
}

impl App {
    pub fn new() -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Self {
            state: AppState::default(),
            action_tx,
            action_rx,
        }
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        self.state.cache.prs = LoadState::Loading;
        self.spawn_load_prs();

        // Draw on the interval tick directly instead of pumping `Render` actions
        // through the unbounded action channel. With `Skip`, a slow frame just
        // delays the next tick — frames can never queue up and back-pressure the
        // app into getting slower and slower.
        let mut interval = time::interval(Duration::from_millis(16));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        let mut events = EventStream::new();
        terminal.draw(|f| render(f, &mut self.state))?;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    terminal.draw(|f| render(f, &mut self.state))?;
                }
                Some(Ok(event)) = events.next() => {
                    if let Event::Key(key) = event
                        && key.kind == KeyEventKind::Press
                            && let Some(action) = key_to_action(&self.state, key.code) {
                                self.action_tx.send(action).ok();
                            }
                }
                Some(action) = self.action_rx.recv() => {
                    match action {
                        Action::Quit => return Ok(()),
                        other => self.apply(other),
                    }
                }
            }
        }
    }
}
