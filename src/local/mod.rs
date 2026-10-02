//! What slussa keeps on disk, shared by the TUI and the headless subcommands.
//! A file's spelling changes only with a new file version.

// A match on one of our own enums names every variant, so that adding one is a
// compile error wherever it has to be handled.
#![cfg_attr(not(test), warn(clippy::wildcard_enum_match_arm))]

pub mod drafts;
pub mod file;
pub mod scope;
