use std::{ops::ControlFlow, time::Duration};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, EventStream, KeyEventKind},
};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task, time,
};
use tokio_stream::StreamExt;

use crate::{
    app::state::{AppState, LoadState, Screen},
    providers::github,
    tui::{Action, key_to_action, pr_detail::DetailTab, render},
};

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
        terminal.draw(|f| render(f, &self.state))?;

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
                            terminal.draw(|f| render(f, &self.state))?;
                        }
                        other => {
                            if self.apply(other).is_break() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    fn apply(&mut self, action: Action) -> ControlFlow<()> {
        match action {
            Action::Quit | Action::Render => unreachable!("handled in run()"),
            Action::Back => {
                self.state.screen = Screen::List;
            }
            Action::NextPr => {
                let len = match &self.state.cache.prs {
                    LoadState::Loaded(prs) => prs.len(),
                    _ => 0,
                };
                let last = len.saturating_sub(1);
                self.state.selected = (self.state.selected + 1).min(last);
            }
            Action::PrevPr => {
                self.state.selected = self.state.selected.saturating_sub(1);
            }
            Action::NextTab => {
                if let Screen::Detail { tab, .. } = &mut self.state.screen {
                    *tab = tab.next();
                }
            }
            Action::PrevTab => {
                if let Screen::Detail { tab, .. } = &mut self.state.screen {
                    *tab = tab.prev();
                }
            }
            Action::OpenPr(pr_id) => {
                self.state.screen = Screen::Detail {
                    pr_id,
                    tab: DetailTab::default(),
                };
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                if matches!(pr_data.commits, LoadState::NotRequested) {
                    pr_data.commits = LoadState::Loading;
                    self.spawn_load_commits(pr_id);
                }
            }
            Action::PrsLoaded(prs) => {
                self.state.cache.prs = LoadState::Loaded(prs);
            }
            Action::CommitsLoaded(pr_id, commits) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.commits = LoadState::Loaded(commits);
            }
        }
        ControlFlow::Continue(())
    }

    fn spawn_load_prs(&self) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let prs = task::spawn_blocking(github::fetch_prs)
                .await
                .unwrap_or_default();
            tx.send(Action::PrsLoaded(prs)).ok();
        });
    }

    fn spawn_load_commits(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let commits = task::spawn_blocking(move || github::fetch_commits(pr_id))
                .await
                .unwrap_or_default();
            tx.send(Action::CommitsLoaded(pr_id, commits)).ok();
        });
    }
}
