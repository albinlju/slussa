//! The application: shared data, effects and the event loop.
//!
//! `event_loop` turns terminal input into `Action`s for the UI and runs the
//! `Effect`s the UI returns. Work that blocks runs off the UI thread
//! (`fetchers`, `desktop`) and comes back as a `TaskResult` (`loads`).

// A match on one of our own enums names every variant, so that adding one is a
// compile error wherever it has to be handled. Tests assert by a catch-all.
#![cfg_attr(not(test), warn(clippy::wildcard_enum_match_arm))]

pub mod commands;
mod desktop;
mod drafts;
pub mod effect;
mod event_loop;
pub mod fetchers;
mod loads;
pub mod navigation;
mod notice;
pub mod refresh;
pub mod state;
pub mod store;
mod terminal;

pub use event_loop::App;
pub(in crate::tui) use terminal::TerminalGuard;

#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod tests;
