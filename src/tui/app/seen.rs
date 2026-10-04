//! What the reader has looked at: a PR is seen when it is opened and while it is
//! on screen, at the latest the list knows of it, so what the reader does
//! themself to a PR (a comment, a merge) does not light it up again.
//! The file itself is `local::seen`.

use chrono::Utc;

use super::{App, navigation::Screen};
use crate::{domain::pr::PrId, local::seen::SeenStorage};

/// Where what was looked at is kept. It is a convenience, so an app starts
/// without it: marks then last this run only.
pub enum SeenFile {
    Disk(SeenStorage),
    /// Another slussa has the file, it cannot be read, or this is a test.
    Unavailable,
}

impl App {
    /// Open the file of what was looked at, and forget the old. Without it the
    /// app goes on, remembering for this run only.
    pub(super) fn open_seen(&mut self, scope: String) {
        let Some(root) = dirs::data_local_dir().map(|dir| dir.join("slussa/seen")) else {
            return;
        };
        match SeenStorage::open(&root, scope) {
            Ok((storage, mut seen)) => {
                seen.forget_old(Utc::now());
                self.state.store.seen = seen;
                self.seen_file = SeenFile::Disk(storage);
            }
            Err(error) => tracing::warn!("not remembering what was looked at: {error}"),
        }
    }

    /// The PR on screen, if any, has been seen as the list last knew it.
    pub(super) fn mark_viewed_seen(&mut self) {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        self.mark_seen(pr_id);
    }

    fn mark_seen(&mut self, pr_id: PrId) {
        let Some(updated) = self.state.store.pr_updated(pr_id) else {
            return;
        };
        if self.state.store.seen.mark(pr_id, updated, Utc::now()) {
            self.seen_dirty = true;
        }
    }

    /// Write what was looked at, if it changed. A failed write gives the file up
    /// for the rest of the run rather than failing on every key.
    pub(super) fn save_seen(&mut self) {
        if !std::mem::take(&mut self.seen_dirty) {
            return;
        }
        let SeenFile::Disk(storage) = &mut self.seen_file else {
            return;
        };
        if let Err(error) = storage.save(&self.state.store.seen) {
            tracing::warn!("not remembering what was looked at: {error}");
            self.seen_file = SeenFile::Unavailable;
        }
    }
}
