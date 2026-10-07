//! Leaving while an agent is reviewing a PR. `q` does not quit then: it asks, since
//! the review is minutes of work that quitting throws away, and once the reader
//! has said yes it waits, with a spinner, for the agent to stop before slussa
//! closes, so that no command is left running with nobody to read it.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::KeyEvent;

use super::{App, event_loop::Next};
use crate::tui::ui::quit_dialog;

/// How long slussa waits for the agents to stop once the reader has said yes. A
/// command is ended at once, so this is only for one that does not die.
const WAIT: Duration = Duration::from_secs(5);

/// The two answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitChoice {
    StopAndQuit,
    KeepRunning,
}

/// What a key means to the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitAnswer {
    /// The other of the two answers is chosen.
    Switch,
    /// What is chosen is taken.
    Take,
    Yes,
    No,
    /// A key that is not an answer; `q` again is one, since asking twice is how a
    /// quit is made by mistake.
    Nothing,
}

impl QuitChoice {
    const fn other(self) -> Self {
        match self {
            Self::StopAndQuit => Self::KeepRunning,
            Self::KeepRunning => Self::StopAndQuit,
        }
    }
}

/// Where leaving is: not asked about, asked, or waiting for the agents to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuitGate {
    #[default]
    Open,
    Asking(QuitChoice),
    Waiting {
        since: Instant,
    },
}

impl App {
    /// `q` was pressed. Quits, unless an agent is reviewing a PR, in which case
    /// it asks first. What is asked about never starts as a yes.
    pub(super) fn quit_or_ask(&mut self) -> Next {
        if self.agent_reviews.is_empty() {
            return self.quit_now();
        }
        self.state.store.quit = QuitGate::Asking(QuitChoice::KeepRunning);
        Next::Continue
    }

    /// Leaving saves the drafts first, and a failed save keeps slussa open with the
    /// reason in the footer: leaving would lose them.
    pub(super) fn quit_now(&mut self) -> Next {
        if self.save_drafts() {
            Next::Quit
        } else {
            self.state.store.quit = QuitGate::Open;
            Next::Continue
        }
    }

    /// A key while the question is asked or the agents are being waited for.
    /// `None` when leaving is not in question and the key is for the screen.
    pub(super) fn quit_key(&mut self, key: KeyEvent) -> Option<Next> {
        match self.state.store.quit {
            QuitGate::Open => None,
            // Nothing to decide any more: the agents are being stopped.
            QuitGate::Waiting { .. } => Some(Next::Continue),
            QuitGate::Asking(choice) => Some(match quit_dialog::answer(key) {
                QuitAnswer::Switch => {
                    self.state.store.quit = QuitGate::Asking(choice.other());
                    Next::Continue
                }
                QuitAnswer::Yes => self.begin_stopping(),
                QuitAnswer::Take if choice == QuitChoice::StopAndQuit => self.begin_stopping(),
                QuitAnswer::Take | QuitAnswer::No => {
                    self.state.store.quit = QuitGate::Open;
                    Next::Continue
                }
                QuitAnswer::Nothing => Next::Continue,
            }),
        }
    }

    /// The reader said yes: every review is told to stop, and slussa waits for them.
    fn begin_stopping(&mut self) -> Next {
        for cancel in self.agent_reviews.values() {
            cancel.cancel();
        }
        self.state.store.quit = QuitGate::Waiting {
            since: Instant::now(),
        };
        Next::Continue
    }

    /// Whether slussa can close now: the reviews have reported that they stopped,
    /// or the wait is over. Checked as the screen is redrawn.
    pub(super) fn waiting_is_over(&mut self) -> bool {
        let QuitGate::Waiting { since } = self.state.store.quit else {
            return false;
        };
        if self.agent_reviews.is_empty() {
            return self.quit_now() == Next::Quit;
        }
        if since.elapsed() >= WAIT {
            tracing::warn!("leaving without waiting any longer for an agent to stop");
            return self.quit_now() == Next::Quit;
        }
        false
    }

    /// For a test: the wait began this long ago.
    #[cfg(test)]
    pub(super) fn waited_for(&mut self, long: Duration) {
        self.state.store.quit = QuitGate::Waiting {
            since: Instant::now()
                .checked_sub(long)
                .unwrap_or_else(Instant::now),
        };
    }
}
