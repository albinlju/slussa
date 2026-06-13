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
    providers::Provider,
    tui::{key_to_action, render},
};

pub mod action;
pub mod fetchers;
pub mod file_tree;
pub mod preflight;
pub mod reducer;
pub mod remote;
pub mod state;

const SPINNER_INTERVAL: Duration = Duration::from_millis(100);

pub struct App {
    pub state: AppState,
    pub(crate) provider: Provider,
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
}

impl App {
    pub fn new(provider: Provider) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Self {
            state: AppState::default(),
            provider,
            action_tx,
            action_rx,
        }
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        self.state.cache.prs = LoadState::Loading;
        self.spawn_load_prs();

        let mut events = EventStream::new();
        self.draw(terminal)?;

        loop {
            let animating = self.state.is_loading();
            tokio::select! {
                () = time::sleep(SPINNER_INTERVAL), if animating => self.draw(terminal)?,
                Some(Ok(event)) = events.next() => match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if let Some(action) = key_to_action(&self.state, key) {
                            self.action_tx.send(action).ok();
                        }
                    }
                    Event::Resize(_, _) => self.draw(terminal)?,
                    _ => {}
                },
                Some(action) = self.action_rx.recv() => match action {
                    Action::Quit => return Ok(()),
                    other => {
                        self.apply(other);
                        self.draw(terminal)?;
                    }
                },
            }
        }
    }

    fn draw(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        terminal.draw(|f| render(f, &mut self.state))?;
        Ok(())
    }
}
