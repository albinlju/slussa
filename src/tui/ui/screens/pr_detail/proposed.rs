//! Which of the agents' proposals the PR's diff shows: those the reader has not
//! dealt with, written against the commit the diff is of. One written against
//! another commit does not belong on these lines, and is only counted.

use crate::{
    domain::pr::PrId,
    tui::{
        app::store::{PrData, Store},
        ui::components::diff_viewer::ProposalAt,
    },
};

/// The head of the PR's own diff, once it is read.
fn diff_head(data: Option<&PrData>) -> Option<&str> {
    let revision = data?.diff.loaded()?.revision.as_ref()?;
    (!revision.commit).then_some(revision.head.as_str())
}

/// The proposals to draw on the PR's diff.
pub fn on_this_diff<'a>(
    store: &'a Store,
    pr_id: PrId,
    data: Option<&PrData>,
) -> Vec<ProposalAt<'a>> {
    let (Some(shown), Some(all)) = (diff_head(data), store.proposals.for_pr(pr_id)) else {
        return Vec::new();
    };
    all.comments
        .iter()
        .enumerate()
        .filter(|(_, proposal)| {
            proposal.head().as_str() == shown && !store.seen.is_handled(pr_id, proposal)
        })
        .map(|(index, proposal)| ProposalAt { index, proposal })
        .collect()
}

/// How many the reader has not dealt with that are written against another
/// commit than the diff's.
pub fn for_another_commit(store: &Store, pr_id: PrId, data: Option<&PrData>) -> usize {
    let (Some(shown), Some(all)) = (diff_head(data), store.proposals.for_pr(pr_id)) else {
        return 0;
    };
    all.comments
        .iter()
        .filter(|proposal| {
            proposal.head().as_str() != shown && !store.seen.is_handled(pr_id, proposal)
        })
        .count()
}
