//! What is new since the reader looked is read in order: the PR's own diff of
//! the head first, the compare after it. The compare holds what a merge of the
//! target brought into the branch too, and is kept to the files of the PR, which
//! only the PR's diff of that very head names. The diff of an older head would
//! drop a file the newer one added, and the head would count as read without it.

use std::collections::HashSet;

use super::App;
use crate::{
    domain::{
        commit::CommitOid,
        diff::{Compared, Diff, DiffRange},
        pr::{PrGroup, PrId},
    },
    providers::error::FetchError,
    tui::app::store::{FetchKey, LoadState, PrResource},
};

const MOVED_AGAIN: &str =
    "The branch moved again while what is new was read. esc, then w: what is new now.";

impl App {
    /// The files the PR touches at `head`: only the PR's diff of that head says.
    fn pr_files_at(&self, pr_id: PrId, head: &CommitOid) -> Option<HashSet<String>> {
        let loaded = CommitOid::parse(self.loaded_diff_head(pr_id)?)?;
        if loaded != *head {
            return None;
        }
        let diff = self.state.store.cache.details.get(&pr_id)?.diff.loaded()?;
        Some(diff.files.iter().map(|file| file.path.clone()).collect())
    }

    /// The commit the PR is against, as the PR's diff of `head` says it.
    pub(super) fn target_at(&self, pr_id: PrId, head: &CommitOid) -> Option<CommitOid> {
        let diff = self.state.store.cache.details.get(&pr_id)?.diff.loaded()?;
        let revision = diff.revision.as_ref().filter(|revision| !revision.commit)?;
        if CommitOid::parse(&revision.head)? != *head {
            return None;
        }
        CommitOid::parse(revision.base.as_deref()?)
    }

    /// Ask for what is new: the compare when the PR's diff of its head is there,
    /// that diff first when it is not; `new_since_after_diff` goes on from it.
    pub(super) fn load_new_since(&mut self, pr_id: PrId, range: DiffRange) {
        if self.pr_files_at(pr_id, &range.head).is_some() {
            self.ensure_loaded(FetchKey::Pr(PrResource::RangeDiff(range), pr_id));
        } else {
            self.spawn_load_diff(pr_id);
        }
    }

    /// The PR's diff was read, or could not be (`failure` says why): what is new
    /// that waited for it is asked for, or says why it cannot be told.
    pub(super) fn new_since_after_diff(&mut self, pr_id: PrId, failure: Option<String>) {
        if !self.shows(pr_id) {
            return;
        }
        let Some(since) = &self.state.ui.detail.since else {
            return;
        };
        let range = since.range().clone();
        let there = self
            .state
            .store
            .cache
            .details
            .get(&pr_id)
            .and_then(|data| data.range_diffs.get(&range))
            .is_some_and(|state| matches!(state, LoadState::Loaded(_) | LoadState::Loading));
        if there {
            return;
        }
        if self.pr_files_at(pr_id, &range.head).is_some() {
            self.ensure_loaded(FetchKey::Pr(PrResource::RangeDiff(range), pr_id));
            return;
        }
        let why = if let Some(message) = failure {
            format!(
                "What is new is kept to the files of the PR, and the PR's diff could not be \
                 read ({message}). F: read it again."
            )
        } else {
            // The list says where the branch is now, which `w` goes by.
            self.spawn_load_prs(PrGroup::Open, None);
            MOVED_AGAIN.to_owned()
        };
        tracing::warn!("what is new not read: pr={pr_id}: no diff of the PR at its head");
        self.state
            .store
            .cache
            .details
            .entry(pr_id)
            .or_default()
            .range_diffs
            .insert(range, LoadState::Failed(FetchError::Stale(why)));
    }

    /// The compare kept to the files of the PR: those it touches at the head, and
    /// those it touched at the commit that was read. A file among the last that it
    /// no longer touches was put back as the target has it, and that is new: left
    /// out, a check that was read and then removed would go unseen. What is left
    /// out is what a merge of the target brought, in files the PR never touched.
    /// When the files it touched before are not known, nothing is left out. A
    /// compare that arrives when the PR's diff is not of its head is not kept,
    /// and nor is one whose head is no longer where the list says the branch is:
    /// what is new ends at the branch as it is, as the whole diff that counts
    /// as read does.
    pub(super) fn new_in_the_pr(
        &self,
        pr_id: PrId,
        range: &DiffRange,
        compared: Result<Compared, FetchError>,
    ) -> Result<Diff, FetchError> {
        let Compared {
            mut diff,
            in_pr_before,
        } = compared?;
        if self.listed_head(pr_id).as_ref() != Some(&range.head) {
            tracing::warn!("what is new not kept: pr={pr_id}: the branch moved on meanwhile");
            return Err(FetchError::Stale(MOVED_AGAIN.into()));
        }
        let Some(now) = self.pr_files_at(pr_id, &range.head) else {
            tracing::warn!("what is new not kept: pr={pr_id}: no diff of the PR at its head");
            return Err(FetchError::Stale(MOVED_AGAIN.into()));
        };
        if let Some(before) = in_pr_before {
            diff.files
                .retain(|file| now.contains(&file.path) || before.contains(&file.path));
        }
        Ok(diff)
    }
}
