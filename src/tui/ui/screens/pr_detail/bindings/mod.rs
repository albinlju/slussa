//! One row per key the PR screen routes itself: which key, where it works,
//! what the help says of it and what it does right now. Routing, the help and
//! the footer's hints for these keys read the same row, so they cannot
//! disagree about where a key works or when it is offered.
//!
//! What stays code: the modal priority at the top of `keys.rs`, the keys a
//! child component routes (`j`/`k`, `enter`, `/`, `n`/`N` ...), which the help
//! names on its own, and which hints each surface shows (`footer.rs`).

use crate::{
    domain::capabilities::Feature,
    tui::ui::{
        action::{Action, DetailAction, PrAction},
        screens::pr_detail::{DetailView, tabs::DetailTab},
        widgets::Hint,
    },
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

mod agent_rows;
pub(super) mod rows;

/// What has to hold of the provider and the PR for a key to be in the help.
#[derive(Clone, Copy)]
pub(super) enum Needs {
    Nothing,
    /// The PR has a web address.
    PrLink,
    /// The provider takes reviews.
    Reviews,
    /// The provider has a way to merge.
    MergeStrategy,
    CloseOrReopen,
    /// A comment of some kind can be written: on the PR, on a line or as a reply.
    AnyComment,
    /// The PR has an AI agent's comment to filter on.
    AiFilter,
    Feature(Feature),
}

/// What a row says about its key, for this provider and this PR right now.
#[derive(Debug)]
pub(super) enum Offer {
    /// There is nothing to do: the provider does not have it, or nothing is in
    /// reach for it to act on. Not shown in the footer, and the key falls
    /// through.
    Hidden,
    /// The PR's state stands in the way. The footer shows it dimmed with the
    /// reason, and the key does nothing.
    Blocked(&'static str),
    /// It works, and does this.
    Offered(Action),
}

/// Which modifiers the key may arrive with. Single letters fire only
/// unmodified, so Ctrl-d and the like (scrolling, muscle memory) do not
/// trigger comment and approve actions; a capital arrives with Shift.
#[derive(Clone, Copy)]
enum Mods {
    Plain,
    Any,
}

impl Mods {
    const fn admits(self, held: KeyModifiers) -> bool {
        match self {
            Self::Plain => held.is_empty(),
            Self::Any => true,
        }
    }
}

/// The tabs a key works on.
#[derive(Clone, Copy)]
enum Place {
    Anywhere,
    /// The tabs that read the PR itself, so a PR can be acted on straight from
    /// its description.
    ReadsPr,
    ReadsPrOrDiff,
    Overview,
    Builds,
}

impl Place {
    const fn holds(self, tab: DetailTab) -> bool {
        match self {
            Self::Anywhere => true,
            Self::ReadsPr => matches!(tab, DetailTab::Overview | DetailTab::Description),
            Self::ReadsPrOrDiff => matches!(
                tab,
                DetailTab::Overview | DetailTab::Description | DetailTab::Diff
            ),
            Self::Overview => matches!(tab, DetailTab::Overview),
            Self::Builds => matches!(tab, DetailTab::Builds),
        }
    }
}

/// How the help writes a key: its cell, and what it does.
pub(super) struct Doc {
    pub keys: &'static str,
    pub text: &'static str,
}

pub(super) struct Binding {
    key: char,
    mods: Mods,
    place: Place,
    /// `None` for a row whose key the help names under another row.
    pub doc: Option<Doc>,
    pub needs: Needs,
    label: Label,
    offer: fn(&DetailView<'_>) -> Offer,
}

/// The footer's hint for a row, key included.
enum Label {
    Fixed(&'static str),
    /// One that carries state, such as how many comments a draft holds.
    Of(fn(&DetailView<'_>) -> String),
}

impl Label {
    fn text(&self, view: &DetailView<'_>) -> String {
        match self {
            Self::Fixed(text) => (*text).to_owned(),
            Self::Of(label) => label(view),
        }
    }
}

impl Binding {
    pub(super) fn offer(&self, view: &DetailView<'_>) -> Offer {
        (self.offer)(view)
    }

    /// The footer's hint for this row: lit when it works, dimmed with the
    /// reason when the PR's state stands in the way, none when it is hidden.
    fn hint(&self, view: &DetailView<'_>) -> Option<Hint> {
        let label = self.label.text(view);
        match self.offer(view) {
            Offer::Hidden => None,
            Offer::Blocked(reason) => Some(Hint::off(format!("{label} ({reason})"))),
            Offer::Offered(_) => Some(Hint::on(label)),
        }
    }
}

/// What the key does on this screen, if a row offers it where the reader is.
pub(super) fn route(view: &DetailView<'_>, key: KeyEvent) -> Option<Action> {
    let KeyCode::Char(typed) = key.code else {
        return None;
    };
    rows::BINDINGS
        .iter()
        .filter(|row| {
            row.key == typed && row.mods.admits(key.modifiers) && row.place.holds(view.tab)
        })
        .find_map(|row| match row.offer(view) {
            Offer::Offered(action) => Some(action),
            Offer::Hidden | Offer::Blocked(_) => None,
        })
}

/// The footer's hint for a key: that of the first row for it which is not
/// hidden. Where the same key does two things (`x` reopens or declines, `v`
/// starts or finishes), the row that is not hidden is the one that applies.
pub(super) fn hint(view: &DetailView<'_>, key: char) -> Option<Hint> {
    rows::BINDINGS
        .iter()
        .filter(|row| row.key == key && row.place.holds(view.tab))
        .find_map(|row| row.hint(view))
}

/// An action that sends a verdict or a merge, which is tied to the commit that
/// was read: offered once there is one, and dimmed with the reason before.
fn offered_on_a_known_head(view: &DetailView<'_>, action: PrAction) -> Offer {
    match offered(view, action) {
        Offer::Offered(_) if view.reviewed_head().is_none() => Offer::Blocked("commit unknown"),
        other => other,
    }
}

/// An action every provider that supports it offers, hidden where it does not.
fn offered(view: &DetailView<'_>, action: PrAction) -> Offer {
    if view.supports_action(DetailAction::Pr(action)) {
        Offer::Offered(Action::from(action))
    } else {
        Offer::Hidden
    }
}
