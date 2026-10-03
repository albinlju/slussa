//! Test doubles for the two transports slussa talks through: the `gh` CLI
//! (`FakeGh`) and Bitbucket's HTTP API (`MockHttp`). Tests built on them drive
//! the real provider, fetcher and store code without a network.
//!
//! `FakeGh` replaces the `gh` program process-wide, so installing one takes a
//! lock and tests that use it run one at a time until the guard is dropped.
//! The fake runs as a script under `/bin/sh`, never as a freshly written
//! executable, which avoids "text file busy" races with tests that spawn
//! processes at the same time.

mod fixtures;
mod gh;
mod http;
mod temp_dir;

pub use fixtures::{gh_closed_pr, gh_list_page, gh_pr};
pub use gh::{FakeGh, InstalledGh, gh_command};
pub use http::{MockHttp, Route};
pub use temp_dir::TempDir;
