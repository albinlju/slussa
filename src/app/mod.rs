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
    app::state::{AppState, DiffViewState, LoadState, Screen},
    providers::github,
    tui::{
        Action, key_to_action,
        pr_detail::{
            DetailTab,
            file_tree::{TreeRow, build_visible_rows},
        },
        render,
    },
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
                self.state.ui.list_selected = (self.state.ui.list_selected + 1).min(last);
            }
            Action::PrevPr => {
                self.state.ui.list_selected = self.state.ui.list_selected.saturating_sub(1);
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
                self.state.ui.diff = DiffViewState::default();
                let (load_commits, load_diff) = {
                    let pr_data = self.state.cache.details.entry(pr_id).or_default();
                    let load_commits = matches!(pr_data.commits, LoadState::NotRequested);
                    let load_diff = matches!(pr_data.diff, LoadState::NotRequested);
                    if load_commits {
                        pr_data.commits = LoadState::Loading;
                    }
                    if load_diff {
                        pr_data.diff = LoadState::Loading;
                    }
                    (load_commits, load_diff)
                };
                if load_commits {
                    self.spawn_load_commits(pr_id);
                }
                if load_diff {
                    self.spawn_load_diff(pr_id);
                }
            }
            Action::PrsLoaded(prs) => {
                self.state.cache.prs = LoadState::Loaded(prs);
            }
            Action::CommitsLoaded(pr_id, commits) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.commits = LoadState::Loaded(commits);
            }
            Action::DiffLoaded(pr_id, diff) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.diff = LoadState::Loaded(diff);
            }
            Action::DiffCursorDown => {
                let rows = self.current_visible_rows();
                let last = rows.len().saturating_sub(1);
                let new_cursor = (self.state.ui.diff.cursor + 1).min(last);
                self.state.ui.diff.cursor = new_cursor;
                if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
                    self.state.ui.diff.focused_file = *file_index;
                }
            }
            Action::DiffCursorUp => {
                let new_cursor = self.state.ui.diff.cursor.saturating_sub(1);
                self.state.ui.diff.cursor = new_cursor;
                let rows = self.current_visible_rows();
                if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
                    self.state.ui.diff.focused_file = *file_index;
                }
            }
            Action::DiffToggleAtCursor => {
                let rows = self.current_visible_rows();
                if let Some(row) = rows.get(self.state.ui.diff.cursor) {
                    match row {
                        TreeRow::Dir {
                            path, expanded, ..
                        } => {
                            if *expanded {
                                self.state.ui.diff.collapsed.insert(path.clone());
                            } else {
                                self.state.ui.diff.collapsed.remove(path);
                            }
                        }
                        TreeRow::File { file_index, .. } => {
                            self.state.ui.diff.focused_file = *file_index;
                        }
                    }
                }
            }
            Action::DiffCollapseAtCursor => {
                let rows = self.current_visible_rows();
                if let Some(TreeRow::Dir { path, .. }) = rows.get(self.state.ui.diff.cursor) {
                    self.state.ui.diff.collapsed.insert(path.clone());
                }
            }
            Action::DiffExpandAtCursor => {
                let rows = self.current_visible_rows();
                if let Some(TreeRow::Dir { path, .. }) = rows.get(self.state.ui.diff.cursor) {
                    self.state.ui.diff.collapsed.remove(path);
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn current_visible_rows(&self) -> Vec<TreeRow> {
        let pr_id = match self.state.screen {
            Screen::Detail { pr_id, .. } => pr_id,
            _ => return Vec::new(),
        };
        let files = self
            .state
            .cache
            .details
            .get(&pr_id)
            .and_then(|d| match &d.diff {
                LoadState::Loaded(diff) => Some(&diff.files[..]),
                _ => None,
            });
        match files {
            Some(files) => build_visible_rows(files, &self.state.ui.diff.collapsed),
            None => Vec::new(),
        }
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

    fn spawn_load_diff(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let diff = task::spawn_blocking(move || github::fetch_diff(pr_id))
                .await
                .unwrap_or_default();
            tx.send(Action::DiffLoaded(pr_id, diff)).ok();
        });
    }
}
