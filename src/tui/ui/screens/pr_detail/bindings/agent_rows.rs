//! The rows for what an agent proposed and for asking one to review: `d` on a
//! proposal, and `A`.

use super::{Binding, Doc, Label, Mods, Needs, Offer, Place, offered};
use crate::{
    domain::{capabilities::Feature, pr::PrStatus},
    tui::ui::action::PrAction,
};

/// `d` on an agent's proposal in the diff discards it.
pub(in crate::tui::ui::screens::pr_detail) static DISCARD_PROPOSAL: Binding = Binding {
    key: 'd',
    mods: Mods::Plain,
    place: Place::ReadsPrOrDiff,
    doc: None,
    needs: Needs::Nothing,
    label: Label::Fixed("d: discard"),
    offer: |view| {
        let proposed = view
            .surface()
            .diff_viewer()
            .is_some_and(|viewer| viewer.focused_proposal().is_some());
        if proposed {
            offered(view, PrAction::DiscardProposal)
        } else {
            Offer::Hidden
        }
    },
};

/// `A` asks the configured agent to review the PR, once the reader has said yes.
/// What it finds becomes proposals; nothing is posted.
pub(in crate::tui::ui::screens::pr_detail) static AGENT_REVIEW: Binding = Binding {
    key: 'A',
    mods: Mods::Any,
    place: Place::ReadsPrOrDiff,
    doc: Some(Doc {
        keys: "A",
        text: "ask an agent to review the PR: its findings become proposals (asks first)",
    }),
    needs: Needs::Feature(Feature::AgentReview),
    label: Label::Of(|view| {
        if view.agent_reviewing() {
            "A: stop review".to_owned()
        } else {
            "A: agent review".to_owned()
        }
    }),
    offer: |view| {
        if view.store.agent_review.is_empty() {
            return Offer::Hidden;
        }
        // One that is running can be stopped, whatever has happened to the PR.
        if view.agent_reviewing() {
            return offered(view, PrAction::OpenAgentReview);
        }
        match view.pr.status {
            PrStatus::Open(_) => offered(view, PrAction::OpenAgentReview),
            PrStatus::Merged => Offer::Blocked("merged"),
            PrStatus::Declined => Offer::Blocked("declined"),
        }
    },
};
