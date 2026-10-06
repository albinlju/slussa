//! Which of the agents' proposals the PR's diff shows: those the reader has not
//! dealt with, written against the commit the diff is of. One written against
//! another commit does not belong on these lines: the Diff tab says how many
//! there are, and they are not counted among those that wait for the reader.

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

/// The commit whose proposals the reader can decide on: the one the PR's diff
/// is of once it is read, and until then where the list says the branch is.
fn head_to_decide<'a>(pr: &'a PullRequest, data: Option<&'a PrData>) -> Option<&'a str> {
    diff_head(data).or(pr.head_oid.as_deref())
}

/// How many proposals wait for the reader: those of the commit the Diff tab
/// shows, which it draws and the reader takes or discards there. One written
/// against another commit is not among them. Nothing can be done with it, so
/// it would wait for ever; the Diff tab says that it is there.
pub fn open_count(store: &Store, pr: &PullRequest, data: Option<&PrData>) -> usize {
    let shown = head_to_decide(pr, data);
    store.proposals.for_pr(pr.id).map_or(0, |held| {
        held.comments
            .iter()
            .filter(|proposal| {
                shown.is_none_or(|shown| proposal.head().as_str() == shown)
                    && !store.seen.is_handled(pr.id, proposal)
            })
            .count()
    })
}

/// What the Overview says of an agent's review: its summary, when it has handed
/// one in, and how many proposals wait.
pub fn ai_review<'a>(
    store: &'a Store,
    pr: &PullRequest,
    data: Option<&PrData>,
) -> Option<crate::tui::ui::screens::pr_detail::tabs::overview::AiReview<'a>> {
    let held = store.proposals.for_pr(pr.id);
    let summary = held.and_then(|held| held.summaries.last());
    let open = open_count(store, pr, data);
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
    data?.own_diff_head()
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

impl super::DetailView<'_> {
    /// Whether an agent is reviewing this PR now.
    pub fn agent_reviewing(&self) -> bool {
        self.store
            .fetches
            .contains(&crate::tui::app::store::FetchKey::Pr(
                crate::tui::app::store::PrResource::AgentReview,
                self.pr_id,
            ))
    }
}
