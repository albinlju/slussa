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
    app::{
        action::Action,
        state::{AppState, LoadState},
    },
    clients::Backend,
    tui::{key_to_action, render},
};

pub mod action;
pub mod auth;
pub mod fetchers;
pub mod preflight;
pub mod reducer;
pub mod state;

pub struct App {
    pub state: AppState,
    pub(crate) backend: Backend,
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
}

impl App {
    pub fn new(backend: Backend) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Self {
            state: AppState::default(),
            backend,
            action_tx,
            action_rx,
        }
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        self.state.cache.prs = LoadState::Loading;
        self.spawn_load_prs();

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
                        && let Some(action) = key_to_action(&self.state, key)
                    {
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
