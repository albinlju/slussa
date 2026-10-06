//! The TUI's part of the drafts: where they are kept, and what the application
//! does with them. Never restores executable commands or authentication. The file
//! itself is `local::drafts`.
use super::App;
use crate::{
    domain::pr::PrId,
    local::{
        drafts::{DraftStorage, Snapshot, adopt_earlier},
        scope::{earlier_scope, scope},
    },
    session::{self, Session},
};
use std::io;

/// Where drafts are kept. Outside tests that is always the disk: an app that
/// "saved" drafts nowhere cannot be built.
pub enum Drafts {
    Disk(DraftStorage),
    /// A test that does not look at the draft file.
    #[cfg(test)]
    Nowhere,
}

impl App {
    /// The application with its draft storage open and what it held restored.
    pub fn open(session: Session) -> io::Result<Self> {
        let remote = session::remote::origin_url().map_err(io::Error::other)?;
        let scope = scope(session.provider(), &remote, session.user())?;
        let earlier = earlier_scope(session.provider(), &remote, session.user())?;
        let root = dirs::data_local_dir()
            .ok_or_else(|| io::Error::other("Cannot locate local data directory"))?
            .join("slussa/drafts");
        let (mut storage, mut snapshot) = DraftStorage::open(&root, scope.clone())?;
        if let Some(earlier) = &earlier {
            snapshot = adopt_earlier(&root, earlier, &mut storage, snapshot);
        }
        let mut app = Self::new(session, Drafts::Disk(storage));
        app.restore(snapshot);
        app.open_proposals(scope.clone());
        app.open_seen(scope, earlier.as_deref());
        Ok(app)
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.state.ui.detail.restore_drafts(snapshot.editors);
        self.state.store.reviews = snapshot.reviews.into_iter().collect();
        for &id in &snapshot.interrupted {
            self.state.store.errors.insert(id, "A previous request was interrupted and may have reached the server. Check the PR before sending it again; nothing was resent automatically.".into());
        }
        self.state.store.uncertain_submissions = snapshot.interrupted;
    }

    /// Give a test's app a draft file, as `open` does.
    #[cfg(test)]
    pub(super) fn restore_drafts(&mut self, storage: DraftStorage, snapshot: Snapshot) {
        self.drafts = Drafts::Disk(storage);
        self.restore(snapshot);
    }

    #[cfg_attr(
        not(test),
        expect(
            clippy::infallible_destructuring_match,
            reason = "`Drafts` has a second variant in test builds"
        )
    )]
    pub(super) fn save_drafts(&mut self) -> bool {
        self.drafts_dirty = false;
        let storage = match &mut self.drafts {
            Drafts::Disk(storage) => storage,
            #[cfg(test)]
            Drafts::Nowhere => return true,
        };
        let snapshot = Snapshot {
            editors: self.state.ui.detail.draft_snapshot(),
            reviews: self
                .state
                .store
                .reviews
                .iter()
                .map(|(id, review)| (*id, review.clone()))
                .collect(),
            interrupted: self
                .state
                .store
                .operations
                .keys()
                .copied()
                .chain(self.state.store.uncertain_submissions.iter().copied())
                .collect(),
        };
        match storage.save(snapshot) {
            Ok(()) => {
                self.state.store.draft_error = None;
                true
            }
            Err(e) => {
                self.state.store.draft_error = Some(format!("Drafts not saved: {e}"));
                false
            }
        }
    }
    /// Journal the uncertain outcome before any remote write can start.
    pub(super) fn checkpoint_submission(&mut self, pr_id: PrId) -> bool {
        if self.save_drafts() {
            return true;
        }
        self.state.store.operations.remove(&pr_id);
        self.state.store.errors.insert(
            pr_id,
            "Not sent: draft recovery data could not be saved. Check local storage and try again."
                .into(),
        );
        false
    }
}
