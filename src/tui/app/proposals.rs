//! What agents have proposed on the PR the reader is looking at: where the file
//! is, reading it, and what the reader does with a proposal. The file itself is
//! `local::proposals`; an agent writes it with `slussa propose import`, and
//! slussa only reads it. Which proposals are dealt with is kept in `seen`.

use std::path::PathBuf;

use super::App;
use crate::{
    domain::{pr::PrId, seen::How},
    local::proposals::{self, Stamp, read_changed},
};

/// Where the proposals for this repository and account are read from.
pub struct ProposalsSource {
    pub(super) root: PathBuf,
    pub(super) scope: String,
    /// Which file was read last: it is read again only when it is another.
    read: Stamp,
}

impl ProposalsSource {
    /// The file for `scope` under `root`, not read yet.
    pub(super) fn of(root: PathBuf, scope: String) -> Self {
        Self {
            root,
            scope,
            read: Stamp::default(),
        }
    }
}

impl App {
    /// Read what agents have proposed from `scope`'s file, from here on.
    pub(super) fn open_proposals(&mut self, scope: String) {
        let Some(root) = proposals::root() else {
            return;
        };
        self.proposals_source = Some(ProposalsSource::of(root, scope));
        self.read_proposals();
    }

    /// Read the file again, if an import has replaced it since it was read. A
    /// file that cannot be read leaves what was read before, and says so in the
    /// log; it is the agent's file, not the reader's.
    pub(super) fn read_proposals(&mut self) {
        let Some(source) = &mut self.proposals_source else {
            return;
        };
        match read_changed(&source.root, &source.scope, &mut source.read) {
            Ok(Some(proposals)) => self.state.store.proposals = proposals,
            Ok(None) => {}
            Err(error) => tracing::warn!("not reading the proposals: {error}"),
        }
    }

    /// The reader has dealt with the proposal: it is not shown again.
    pub(super) fn handle_proposal(&mut self, pr_id: PrId, index: usize, how: How) {
        let store = &mut self.state.store;
        let Some(proposal) = store
            .proposals
            .for_pr(pr_id)
            .and_then(|held| held.comments.get(index))
        else {
            return;
        };
        if store.seen.handle(pr_id, proposal, how) {
            self.seen_dirty = true;
        }
    }
}
