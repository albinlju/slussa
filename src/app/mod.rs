//! The application: shared data, effects and the event loop.
//!
//! `event_loop` turns terminal input into `Action`s for the UI and runs the
//! `Effect`s the UI returns. Work that blocks runs off the UI thread
//! (`fetchers`, `desktop`) and comes back as a `TaskResult` (`loads`).

pub mod action;
mod commands;
mod desktop;
mod drafts;
mod event_loop;
pub mod fetchers;
mod loads;
pub mod navigation;
pub mod preflight;
pub mod refresh;
pub mod remote;
pub mod reviews;
pub mod state;
pub mod store;

pub use event_loop::App;

#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod tests;
