//! The interactive program: `app` does the I/O and never draws, `ui` draws and
//! never starts I/O.

pub mod app;
mod run;
pub mod ui;

pub use run::run;
