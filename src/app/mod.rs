use crate::{
    app::{action::Action, state::AppState, store::LoadState},
    domain::pr::PrGroup,
    providers::Provider,
    tui::{key_to_action, render},
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, EventStream, KeyEventKind},
};
use std::time::{Duration, Instant};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time,
};
use tokio_stream::StreamExt;

pub mod action;
mod commands;
mod desktop;
mod drafts;
pub mod fetchers;
mod loads;
pub mod navigation;
pub mod preflight;
pub mod refresh;
pub mod remote;
pub mod reviews;
pub mod state;
pub mod store;

const SPINNER_INTERVAL: Duration = Duration::from_millis(100);
/// How long typed text may wait before it is written to the draft file.
const DRAFT_SAVE_INTERVAL: Duration = Duration::from_millis(500);

pub struct App {
    drafts: Option<drafts::DraftStorage>,
    /// Text was typed since the draft file was last written.
    drafts_dirty: bool,
    pub state: AppState,
    pub(crate) provider: Provider,
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
    pub(crate) full_refreshed: Instant,
}

impl App {
    pub fn new(provider: Provider, current_user: String) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
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
            action_tx,
            action_rx,
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
                Some(event) = events.next() => match event? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if self.handle_key(key) { return Ok(()); }
                        self.draw(terminal)?;
                    }
                    Event::Paste(text) => { self.apply(Action::Paste(text)); self.draw(terminal)?; }
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

    /// Apply input before translating the next key; commands must target the
    /// selection/dialog state produced by all preceding input.
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key_to_action(&self.state, key) {
            Some(Action::Quit) => self.save_drafts(),
            Some(action) => {
                self.apply(action);
                false
            }
            None => false,
        }
    }

    fn draw(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        terminal.draw(|f| render(f, &mut self.state))?;
        Ok(())
    }
}

#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod tests;

impl App {
    /// A keystroke in the editor only marks the drafts as changed, and the event
    /// loop writes them within `DRAFT_SAVE_INTERVAL`; a write is a file sync,
    /// too slow to do for every character. Everything else is written at once.
    pub(super) fn apply(&mut self, action: Action) {
        let typing = action.is_editor_keystroke();
        self.apply_inner(action);
        if typing {
            self.drafts_dirty = true;
        } else {
            self.save_drafts();
        }
    }

    #[expect(
        clippy::unreachable,
        reason = "a component consumed these; the `Action` split removes the arm (ROADMAP)"
    )]
    fn apply_inner(&mut self, action: Action) {
        let Some(action) = self
            .state
            .ui
            .update(action, &self.state.store, self.state.screen)
        else {
            return;
        };
        match action {
            Action::Quit => unreachable!("handled in run()"),
            Action::Navigate(screen) => self.state.screen = screen,
            Action::Refresh => self.refresh_actions(),
            Action::PrLink { pr_id, kind } => self.pr_link(pr_id, kind),
            Action::LinkFinished(result) => {
                self.state.store.link_pending = false;
                self.state.store.notice = Some(match result {
                    Ok(message) => store::Notice::new(message, false),
                    Err(message) => store::Notice::new(message, true),
                });
            }
            Action::List(action::ListAction::OpenPr(id)) => self.open_pr(id),
            Action::List(action::ListAction::LoadOlder) => self.load_older_prs(),
            Action::List(action::ListAction::FilterChanged) => self.ensure_view_loaded(),
            Action::Command { pr_id, command } => self.execute(pr_id, command),
            Action::LoadCommitDiff { pr_id, oid } => self.ensure_commit_diff(pr_id, oid),
            Action::Loaded(a) => self.loaded_actions(a),
            Action::Paste(_)
            | Action::HelpScroll(_)
            | Action::Detail(_)
            | Action::List(_)
            | Action::Diff(_)
            | Action::Commits(_)
            | Action::Search(_) => {
                unreachable!("local action consumed by component")
            }
        }
    }
}
