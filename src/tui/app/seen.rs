//! What the reader has looked at: a PR is seen when it is opened and while it is
//! on screen, at the latest the list knows of it, so what the reader does
//! themself to a PR (a comment, a merge) does not light it up again.
//! The file itself is `local::seen`.

use chrono::Utc;

use super::{App, navigation::Screen};
use crate::tui::ui::screens::pr_detail::tabs::DetailTab;
use crate::{
    domain::{commit::CommitOid, pr::PrId},
    local::seen::{SeenStorage, adopt_earlier},
};

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
    pub(super) fn open_seen(&mut self, scope: String, earlier: Option<&str>) {
        let Some(root) = dirs::data_local_dir().map(|dir| dir.join("slussa/seen")) else {
            return;
        };
        match SeenStorage::open(&root, scope) {
            Ok((mut storage, mut seen)) => {
                if let Some(earlier) = earlier {
                    seen = adopt_earlier(&root, earlier, &mut storage, seen);
                }
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

    /// Whether `pr_id` is the PR on screen.
    pub(super) fn shows(&self, pr_id: PrId) -> bool {
        matches!(self.state.screen, Screen::Detail { pr_id: shown, .. } if shown == pr_id)
    }

    /// The head of the PR's own diff, as read.
    pub(super) fn loaded_diff_head(&self, pr_id: PrId) -> Option<&str> {
        let diff = self.state.store.cache.details.get(&pr_id)?.diff.loaded()?;
        let revision = diff.revision.as_ref()?;
        (!revision.commit).then_some(revision.head.as_str())
    }

    /// The reader has arrived at a diff: the PR's own on the Diff tab, or what is
    /// new since they looked when that is shown. Its head is kept, so that the
    /// next time the PR says what has moved since.
    ///
    /// Only an arrival counts: opening the PR on the Diff, choosing the tab, the
    /// diff the reader waited for there being read, or what is new being shown.
    /// Not a refresh under a reader who is already there, whether it brings a
    /// newer diff or the same one again, which would clear the mark before they
    /// had seen anything, and not a key pressed on the diff. The PR's own diff counts
    /// only while it is the branch as the list has it: after a push it is still on
    /// screen for a while, and having it open is not having read what was pushed.
    pub(super) fn mark_read_head(&mut self) {
        let Screen::Detail {
            pr_id,
            tab: DetailTab::Diff,
        } = self.state.screen
        else {
            return;
        };
        let store = &mut self.state.store;
        let listed = store
            .cache
            .prs
            .loaded()
            .and_then(|prs| prs.iter().find(|pr| pr.id == pr_id))
            .and_then(|pr| pr.head_oid.as_deref())
            .and_then(CommitOid::parse);
        let data = store.cache.details.get(&pr_id);
        let head = match &self.state.ui.detail.since {
            Some(since) => since.head_read(data).cloned(),
            None => data
                .and_then(|data| data.diff.loaded())
                .and_then(|diff| diff.revision.as_ref())
                .filter(|revision| !revision.commit)
                .and_then(|revision| CommitOid::parse(&revision.head))
                .filter(|head| listed.as_ref() == Some(head)),
        };
        if let Some(head) = head
            && store.seen.mark_head(pr_id, &head)
        {
            self.seen_dirty = true;
        }
    }

    fn mark_seen(&mut self, pr_id: PrId) {
        let Some(updated) = self.state.store.pr_updated(pr_id) else {
            return;
        };
        if self.state.store.seen.mark(pr_id, updated, Utc::now()) {
            self.seen_dirty = true;
        }
    }

    /// What the key or the result just taken in may have changed on screen is
    /// looked at, and what was looked at is written.
    pub(super) fn note_seen(&mut self) {
        self.mark_viewed_seen();
        self.save_seen();
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
