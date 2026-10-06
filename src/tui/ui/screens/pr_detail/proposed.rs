//! Which of the agents' proposals the PR's diff shows: those the reader has not
//! dealt with, written against the commit the diff is of. One written against
//! another commit does not belong on these lines, and is only counted.

use crate::{
    domain::{
        commit::CommitOid,
        pr::{PrId, PullRequest},
        proposal::Summary,
    },
    tui::{
        app::store::{PrData, Store},
        ui::components::diff_viewer::ProposalAt,
    },
};

/// How many proposals the reader has not dealt with, whatever commit they are
/// written against.
pub fn open_count(store: &Store, pr_id: PrId) -> usize {
    store.proposals.for_pr(pr_id).map_or(0, |held| {
        held.comments
            .iter()
            .filter(|proposal| !store.seen.is_handled(pr_id, proposal))
            .count()
    })
}

/// What the Overview says of an agent's review: its summary, when it has handed
/// one in, and how many proposals wait.
pub fn ai_review<'a>(
    store: &'a Store,
    pr: &PullRequest,
) -> Option<crate::tui::ui::screens::pr_detail::tabs::overview::AiReview<'a>> {
    let held = store.proposals.for_pr(pr.id);
    let summary = held.and_then(|held| held.summaries.last());
    let open = open_count(store, pr.id);
    if summary.is_none() && open == 0 {
        return None;
    }
    let listed = pr.head_oid.as_deref().and_then(CommitOid::parse);
    Some(
        crate::tui::ui::screens::pr_detail::tabs::overview::AiReview {
            agent: summary.and_then(Summary::agent),
            text: summary.map(Summary::text),
            older: summary
                .is_some_and(|summary| listed.is_some_and(|listed| *summary.head() != listed)),
            open,
        },
    )
}

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
