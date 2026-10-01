use crate::{
    app::{
        action::{Action, Effect, TaskResult},
        drafts, refresh,
        state::AppState,
        store::{self, FetchKey, LoadState},
    },
    domain::pr::PrGroup,
    providers::Provider,
    tui::{key_to_action, render},
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, EventStream, KeyEvent, KeyEventKind},
};
use std::time::{Duration, Instant};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time,
};
use tokio_stream::StreamExt;

const SPINNER_INTERVAL: Duration = Duration::from_millis(100);
/// How long typed text may wait before it is written to the draft file.
const DRAFT_SAVE_INTERVAL: Duration = Duration::from_millis(500);

pub struct App {
    pub(super) drafts: Option<drafts::DraftStorage>,
    /// Text was typed since the draft file was last written.
    pub(super) drafts_dirty: bool,
    pub state: AppState,
    pub(crate) provider: Provider,
    pub(super) results_tx: UnboundedSender<TaskResult>,
    pub(super) results_rx: UnboundedReceiver<TaskResult>,
    pub(crate) full_refreshed: Instant,
}

/// Whether the event loop goes on after an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Next {
    Continue,
    Quit,
}

impl App {
    pub fn new(provider: Provider, current_user: String) -> Self {
        let (results_tx, results_rx) = mpsc::unbounded_channel();
        Self {
            drafts: None,
            drafts_dirty: false,
            state: AppState {
                store: store::Store {
                    current_user,
                    capabilities: provider.capabilities(),
                    ..store::Store::default()
                },
                ..AppState::default()
            },
            provider,
            results_tx,
            results_rx,
            full_refreshed: Instant::now(),
        }
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let result = self.event_loop(terminal).await;
        // Leaving on a terminal error must not lose text typed since the last save.
        if self.drafts_dirty {
            self.save_drafts();
        }
        result
    }

    async fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        self.state.store.cache.prs = LoadState::Loading;
        self.spawn_load_prs(PrGroup::Open, None);

        let mut events = EventStream::new();
        let start = time::Instant::now() + refresh::BUILDS_INTERVAL;
        let mut refresh = time::interval_at(start, refresh::BUILDS_INTERVAL);
        let mut draft_save = time::interval(DRAFT_SAVE_INTERVAL);
        draft_save.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        self.draw(terminal)?;

        loop {
            let animating = self.state.store.link_pending
                || self.state.is_loading()
                || self
                    .state
                    .store
                    .notice
                    .as_ref()
                    .is_some_and(store::Notice::visible);
            tokio::select! {
                () = time::sleep(SPINNER_INTERVAL), if animating => self.draw(terminal)?,
                _ = refresh.tick() => {
                    self.tick_refresh();
                    self.draw(terminal)?;
                }
                _ = draft_save.tick(), if self.drafts_dirty => {
                    self.save_drafts();
                    self.draw(terminal)?;
                }
                Some(event) = events.next() => {
                    let next = match event? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key),
                        Event::Paste(text) => self.apply(Action::Paste(text)),
                        Event::Resize(_, _) => Next::Continue,
                        Event::Key(_)
                        | Event::FocusGained
                        | Event::FocusLost
                        | Event::Mouse(_) => continue,
                    };
                    if next == Next::Quit {
                        return Ok(());
                    }
                    self.draw(terminal)?;
                }
                Some(result) = self.results_rx.recv() => {
                    self.apply_result(result);
                    self.draw(terminal)?;
                }
            }
        }
    }

    /// Apply input before translating the next key; commands must target the
    /// selection/dialog state produced by all preceding input.
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Next {
        match key_to_action(&self.state, key) {
            Some(action) => self.apply(action),
            None => Next::Continue,
        }
    }

    fn draw(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        terminal.draw(|f| render(f, &mut self.state))?;
        Ok(())
    }

    /// Hand input to the UI and run the effect it asks for. A keystroke in the
    /// editor only marks the drafts as changed, and the event loop writes them
    /// within `DRAFT_SAVE_INTERVAL`; a write is a file sync, too slow to do for
    /// every character. Everything else is written at once.
    pub(super) fn apply(&mut self, action: Action) -> Next {
        let typing = action.is_editor_keystroke();
        let effect = self
            .state
            .ui
            .update(action, &self.state.store, self.state.screen);
        let next = effect.map_or(Next::Continue, |effect| self.run_effect(effect));
        if typing {
            self.drafts_dirty = true;
        } else if next == Next::Continue {
            self.save_drafts();
        }
        next
    }

    /// Take in what work off the UI thread sent back.
    pub(super) fn apply_result(&mut self, result: TaskResult) {
        match result {
            TaskResult::Read(read) => self.apply_read(read),
            TaskResult::Written { ticket, result } => self.apply_write(&ticket, result),
            TaskResult::LinkFinished(result) => {
                self.state.store.link_pending = false;
                self.state.store.notice = Some(match result {
                    Ok(message) => store::Notice::new(message, false),
                    Err(message) => store::Notice::new(message, true),
                });
            }
        }
        self.save_drafts();
    }

    fn run_effect(&mut self, effect: Effect) -> Next {
        match effect {
            // Quitting saves the drafts first, and a failed save keeps slussa
            // open with the reason in the footer: leaving would lose them.
            Effect::Quit => {
                return if self.save_drafts() {
                    Next::Quit
                } else {
                    Next::Continue
                };
            }
            Effect::Navigate(screen) => self.state.screen = screen,
            Effect::Refresh => self.refresh_actions(),
            Effect::OpenPr(id) => self.open_pr(id),
            Effect::LoadOlder => self.load_older_prs(),
            Effect::LoadView => self.ensure_view_loaded(),
            Effect::LoadCommitDiff { pr_id, oid } => {
                self.ensure_loaded(FetchKey::CommitDiff(pr_id, oid));
            }
            Effect::DismissError { pr_id } => {
                self.state.store.errors.remove(&pr_id);
            }
            Effect::PrLink { pr_id, kind } => self.pr_link(pr_id, kind),
            Effect::Command { pr_id, command } => self.execute(pr_id, command),
        }
        Next::Continue
    }
}
