//! The rows: one per key, in the order the keys are tried in.

use super::{Binding, Doc, Label, Mods, Needs, Offer, Place, offered, offered_on_a_known_head};
use crate::{
    domain::{capabilities::Feature, pr::PrStatus},
    tui::{
        app::effect::{Effect, LinkAction},
        ui::{
            action::{Action, PrAction, TimelineAction},
            screens::pr_detail::{DetailView, tabs::overview::offers_filter},
        },
    },
};

// The rows, in the order the help lists them. Where a key has two rows, the
// first one that is offered is the one that applies.

pub(in crate::tui::ui::screens::pr_detail) static OPEN_IN_BROWSER: Binding = Binding {
    key: 'o',
    mods: Mods::Plain,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "o",
        text: "open PR in browser",
    }),
    needs: Needs::PrLink,
    label: Label::Fixed("o: open"),
    offer: |view| link(view, LinkAction::Open),
};

pub(in crate::tui::ui::screens::pr_detail) static COPY_LINK: Binding = Binding {
    key: 'y',
    mods: Mods::Plain,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "y",
        text: "copy PR link",
    }),
    needs: Needs::PrLink,
    label: Label::Fixed("y: copy link"),
    offer: |view| link(view, LinkAction::Copy),
};

const fn link(view: &DetailView<'_>, kind: LinkAction) -> Offer {
    if view.has_pr_link() {
        Offer::Offered(Action::Effect(Effect::PrLink {
            pr_id: view.pr_id,
            kind,
        }))
    } else {
        Offer::Hidden
    }
}

/// `a` opens the review-verdict menu. Even on your own PR you can leave a
/// comment review; the picker dims the verdicts you cannot use.
pub(in crate::tui::ui::screens::pr_detail) static SUBMIT_REVIEW: Binding = Binding {
    key: 'a',
    mods: Mods::Plain,
    place: Place::ReadsPr,
    doc: Some(Doc {
        keys: "a",
        text: "submit review",
    }),
    needs: Needs::Reviews,
    label: Label::Fixed("a: submit review"),
    offer: |view| offered_on_a_known_head(view, PrAction::OpenReviewPicker),
};

/// `v` finishes the review in progress: it opens the verdict menu. It works on
/// every tab, since line comments queue into the review from the Diff too.
pub(in crate::tui::ui::screens::pr_detail) static FINISH_REVIEW: Binding = Binding {
    key: 'v',
    mods: Mods::Plain,
    place: Place::Anywhere,
    doc: None,
    needs: Needs::Reviews,
    label: Label::Of(|view| {
        let queued = view
            .pending_review()
            .map_or(0, |review| review.comments.len());
        format!("v: finish draft ({queued})")
    }),
    offer: |view| match view.pending_review() {
        Some(_) => offered_on_a_known_head(view, PrAction::FinishReview),
        None => Offer::Hidden,
    },
};

/// `v` starts a review. Line comments made while it is open queue into it
/// instead of posting.
pub(in crate::tui::ui::screens::pr_detail) static START_REVIEW: Binding = Binding {
    key: 'v',
    mods: Mods::Plain,
    place: Place::ReadsPrOrDiff,
    doc: Some(Doc {
        keys: "v",
        text: "start/finish review draft",
    }),
    needs: Needs::Reviews,
    label: Label::Fixed("v: start review"),
    offer: |view| offered(view, PrAction::StartReview),
};

/// `V` discards a review in progress and its queued comments.
pub(in crate::tui::ui::screens::pr_detail) static DISCARD_REVIEW: Binding = Binding {
    key: 'V',
    mods: Mods::Any,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "V",
        text: "discard review",
    }),
    needs: Needs::Reviews,
    label: Label::Fixed("V: discard"),
    offer: |view| match view.pending_review() {
        Some(_) => offered(view, PrAction::AbandonReview),
        None => Offer::Hidden,
    },
};

/// `m` opens the merge-strategy menu while the PR can be merged. Your own PR
/// can be merged too, so unlike `a` there is no gate on its author.
pub(in crate::tui::ui::screens::pr_detail) static MERGE: Binding = Binding {
    key: 'm',
    mods: Mods::Plain,
    place: Place::ReadsPr,
    doc: Some(Doc {
        keys: "m",
        text: "merge",
    }),
    needs: Needs::MergeStrategy,
    label: Label::Fixed("m: merge"),
    offer: |view| {
        if view.store.capabilities.merge_strategies.is_empty() {
            return Offer::Hidden;
        }
        match view.merge_blocked_reason() {
            Some(reason) => Offer::Blocked(reason),
            None => offered_on_a_known_head(view, PrAction::OpenMergePicker),
        }
    },
};

/// `x` reopens a PR that was declined, with a confirm.
pub(in crate::tui::ui::screens::pr_detail) static REOPEN: Binding = Binding {
    key: 'x',
    mods: Mods::Plain,
    place: Place::ReadsPr,
    doc: Some(Doc {
        keys: "x",
        text: "close / decline, or reopen a declined PR",
    }),
    needs: Needs::CloseOrReopen,
    label: Label::Fixed("x: reopen"),
    offer: |view| {
        if view.pr_is_declined() {
            offered(view, PrAction::OpenReopen)
        } else {
            Offer::Hidden
        }
    },
};

/// `x` declines or closes a PR while it is open, with a confirm. Once it is not,
/// the key stays in the footer, dimmed with why.
pub(in crate::tui::ui::screens::pr_detail) static DECLINE: Binding = Binding {
    key: 'x',
    mods: Mods::Plain,
    place: Place::ReadsPr,
    doc: None,
    needs: Needs::CloseOrReopen,
    label: Label::Fixed("x: decline"),
    offer: |view| {
        if !view.store.capabilities.supports(Feature::ClosePr) {
            return Offer::Hidden;
        }
        match view.decline_blocked_reason() {
            Some(reason) => Offer::Blocked(reason),
            None => Offer::Offered(Action::from(PrAction::OpenDecline)),
        }
    },
};

pub(in crate::tui::ui::screens::pr_detail) static COMMENT: Binding = Binding {
    key: 'c',
    mods: Mods::Plain,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "c",
        text: "comment",
    }),
    needs: Needs::AnyComment,
    label: Label::Fixed("c: comment"),
    offer: |view| offered(view, PrAction::OpenComment),
};

pub(in crate::tui::ui::screens::pr_detail) static REPLY: Binding = Binding {
    key: 'r',
    mods: Mods::Plain,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "r",
        text: "reply",
    }),
    needs: Needs::Feature(Feature::Replies),
    label: Label::Fixed("r: reply"),
    offer: |view| offered(view, PrAction::OpenReply),
};

/// `R` toggles resolve on the focused thread; off a thread it does nothing.
pub(in crate::tui::ui::screens::pr_detail) static RESOLVE_THREAD: Binding = Binding {
    key: 'R',
    mods: Mods::Any,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "R",
        text: "resolve thread",
    }),
    needs: Needs::Feature(Feature::ResolveThreads),
    label: Label::Of(|view| {
        let resolved = view.focused_thread().is_some_and(|thread| thread.resolved);
        if resolved {
            "R: reopen thread".to_owned()
        } else {
            "R: resolve thread".to_owned()
        }
    }),
    offer: |view| offered(view, PrAction::ResolveThread),
};

pub(in crate::tui::ui::screens::pr_detail) static REFRESH: Binding = Binding {
    key: 'F',
    mods: Mods::Any,
    place: Place::Anywhere,
    doc: Some(Doc {
        keys: "F",
        text: "refresh",
    }),
    needs: Needs::Nothing,
    label: Label::Fixed("F: refresh"),
    offer: |_| Offer::Offered(Action::Effect(Effect::Refresh)),
};

/// `f` cycles which comments the Overview shows, once there is an agent's to
/// tell from people's.
pub(in crate::tui::ui::screens::pr_detail) static FILTER_COMMENTS: Binding = Binding {
    key: 'f',
    mods: Mods::Plain,
    place: Place::Overview,
    doc: Some(Doc {
        keys: "f",
        text: "show all / people's / AI comments (Overview)",
    }),
    needs: Needs::AiFilter,
    label: Label::Of(|view| {
        format!(
            "f: comments ({})",
            view.detail.overview.timeline.filter.label()
        )
    }),
    offer: |view| {
        if offers_filter(view.data, view.detail.overview.timeline.filter) {
            Offer::Offered(Action::from(TimelineAction::CycleFilter))
        } else {
            Offer::Hidden
        }
    },
};

pub(in crate::tui::ui::screens::pr_detail) static TOGGLE_FOLD: Binding = Binding {
    key: ' ',
    mods: Mods::Plain,
    place: Place::Overview,
    doc: Some(Doc {
        keys: "space",
        text: "toggle fold: a folder, a resolved thread, a long comment (its fold row)",
    }),
    needs: Needs::Nothing,
    label: Label::Fixed("space: fold"),
    offer: |_| Offer::Offered(Action::from(TimelineAction::ToggleFold)),
};

/// `e` edits the comment the cursor is on; the application checks that it is
/// the reader's own.
pub(in crate::tui::ui::screens::pr_detail) static EDIT_COMMENT: Binding = Binding {
    key: 'e',
    mods: Mods::Plain,
    place: Place::Overview,
    doc: Some(Doc {
        keys: "e",
        text: "edit own",
    }),
    needs: Needs::Feature(Feature::EditComments),
    label: Label::Fixed("e: edit"),
    offer: |view| offered(view, PrAction::EditComment),
};

/// `d` deletes the reader's own comment the cursor is on.
pub(in crate::tui::ui::screens::pr_detail) static DELETE_COMMENT: Binding = Binding {
    key: 'd',
    mods: Mods::Plain,
    place: Place::Overview,
    doc: None,
    needs: Needs::Feature(Feature::DeleteComments),
    label: Label::Fixed("d: delete"),
    offer: |view| offered(view, PrAction::DeleteComment),
};

/// `d` on a comment queued into the review, in the diff pane, takes it out.
pub(in crate::tui::ui::screens::pr_detail) static REMOVE_PENDING: Binding = Binding {
    key: 'd',
    mods: Mods::Plain,
    place: Place::Anywhere,
    doc: None,
    needs: Needs::Reviews,
    label: Label::Fixed("d: remove pending"),
    offer: |view| {
        let queued = view
            .surface()
            .diff_viewer()
            .is_some_and(|viewer| viewer.focused_pending().is_some());
        if queued {
            offered(view, PrAction::RemovePendingComment)
        } else {
            Offer::Hidden
        }
    },
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

/// `b` on the Builds tab runs the failed builds again, when there are some.
pub(in crate::tui::ui::screens::pr_detail) static RERUN_BUILDS: Binding = Binding {
    key: 'b',
    mods: Mods::Plain,
    place: Place::Builds,
    doc: Some(Doc {
        keys: "b",
        text: "run failed builds again (Builds)",
    }),
    needs: Needs::Feature(Feature::RerunBuilds),
    label: Label::Fixed("b: run failed again"),
    offer: |view| {
        if !view.has_failed_build() {
            return Offer::Hidden;
        }
        match view.pr.status {
            PrStatus::Open(_) => offered(view, PrAction::RerunBuilds),
            PrStatus::Merged => Offer::Blocked("merged"),
            PrStatus::Declined => Offer::Blocked("declined"),
        }
    },
};

/// `i` in the Overview opens the issue the PR closes in the browser, or with
/// several asks which. It is what the PR was asked to do, so reading it is the
/// intent check.
pub(in crate::tui::ui::screens::pr_detail) static OPEN_ISSUE: Binding = Binding {
    key: 'i',
    mods: Mods::Plain,
    place: Place::Overview,
    doc: Some(Doc {
        keys: "i",
        text: "open the issue the PR closes in the browser (Overview)",
    }),
    // The issues are read with the description and labels, where a provider does.
    needs: Needs::Feature(Feature::PrInfo),
    label: Label::Of(|view| match view.issues_to_open().as_slice() {
        [] => "i: open issue".to_owned(),
        [(issue, _)] => format!("i: open #{}", issue.number),
        several => format!("i: open issue ({})", several.len()),
    }),
    offer: |view| match view.issues_to_open().as_slice() {
        [] => Offer::Hidden,
        [(issue, url)] => Offer::Offered(Action::Effect(Effect::IssueLink {
            number: issue.number,
            url: (*url).to_owned(),
        })),
        [_, _, ..] => offered(view, PrAction::OpenIssues),
    },
};

/// `p` asks those who asked for changes to look again, once there are some.
pub(in crate::tui::ui::screens::pr_detail) static REREQUEST_REVIEW: Binding = Binding {
    key: 'p',
    mods: Mods::Plain,
    place: Place::ReadsPr,
    doc: Some(Doc {
        keys: "p",
        text: "ask those who asked for changes to review again",
    }),
    needs: Needs::Feature(Feature::RerequestReview),
    label: Label::Of(|view| match view.rerequest() {
        Some(who) => match who.names() {
            [only] => format!("p: ask {only} again"),
            [first, rest @ ..] => format!("p: ask {first} +{} again", rest.len()),
            [] => "p: ask again".to_owned(),
        },
        None => "p: ask again".to_owned(),
    }),
    offer: |view| {
        if view.rerequest().is_none() {
            return Offer::Hidden;
        }
        match view.pr.status {
            PrStatus::Open(_) => offered(view, PrAction::RerequestReview),
            PrStatus::Merged => Offer::Blocked("merged"),
            PrStatus::Declined => Offer::Blocked("declined"),
        }
    },
};

/// The order is the order the keys are tried in.
pub(in crate::tui::ui::screens::pr_detail) static BINDINGS: &[&Binding] = &[
    &OPEN_IN_BROWSER,
    &COPY_LINK,
    &SUBMIT_REVIEW,
    &FINISH_REVIEW,
    &START_REVIEW,
    &DISCARD_REVIEW,
    &MERGE,
    &REOPEN,
    &DECLINE,
    &COMMENT,
    &REPLY,
    &RESOLVE_THREAD,
    &REFRESH,
    &FILTER_COMMENTS,
    &TOGGLE_FOLD,
    &EDIT_COMMENT,
    &DELETE_COMMENT,
    &REMOVE_PENDING,
    &DISCARD_PROPOSAL,
    &RERUN_BUILDS,
    &REREQUEST_REVIEW,
    &OPEN_ISSUE,
];
