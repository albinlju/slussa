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

        let render_tx = self.action_tx.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(16));
            loop {
                interval.tick().await;
                if render_tx.send(Action::Render).is_err() {
                    break;
                }
            }
        });

        let mut events = EventStream::new();
        terminal.draw(|f| render(f, &mut self.state))?;

        loop {
            tokio::select! {
                Some(Ok(event)) = events.next() => {
                    if let Event::Key(key) = event {
                        if key.kind == KeyEventKind::Press {
                            if let Some(action) = key_to_action(&self.state, key.code) {
                                self.action_tx.send(action).ok();
                            }
                        }
                    }
                }
                Some(action) = self.action_rx.recv() => {
                    match action {
                        Action::Quit => return Ok(()),
                        Action::Render => {
                            terminal.draw(|f| render(f, &mut self.state))?;
                        }
                        other => self.apply(other),
                    }
                }
            }
        }
    }
}
