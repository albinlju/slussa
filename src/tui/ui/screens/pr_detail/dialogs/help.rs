//! The PR view's help: every key, with what has to hold for it to be offered.
//! A key the provider does not have is left out, not greyed out (AGENTS.md,
//! *Show only what the provider supports*).
use crate::{
    domain::capabilities::{Capabilities, Feature},
    tui::ui::screens::pr_detail::bindings::{self, Binding, Needs},
};

/// One line of the help.
#[derive(Clone, Copy)]
enum Entry {
    /// Shown as written where its need holds.
    Key(&'static str, &'static str, Needs),
    /// A key the PR screen routes through a row of `bindings`, which says how
    /// the help writes it.
    Bound(&'static Binding),
    /// The tab numbers, one fewer where the provider has no builds.
    Tabs,
    /// `d` deletes a comment of one's own, removes a queued review comment, or
    /// both.
    Delete,
}

use Entry::Key;

const HELP: &[Entry] = &[
    Entry::Bound(&bindings::rows::OPEN_IN_BROWSER),
    Entry::Bound(&bindings::rows::COPY_LINK),
    Key("j/k", "move up/down", Needs::Nothing),
    Key("^d/^u", "half-page", Needs::Nothing),
    Key("h/l", "previous / next tab", Needs::Nothing),
    Key("H/L", "pan wide Description", Needs::Nothing),
    Entry::Tabs,
    Key("enter", "open / view", Needs::Nothing),
    Entry::Bound(&bindings::rows::TOGGLE_FOLD),
    Key("/", "search", Needs::Nothing),
    Key("n/N", "next/prev match", Needs::Nothing),
    Key("[ ]", "prev/next tab/commit", Needs::Nothing),
    Key("esc", "back", Needs::Nothing),
    Entry::Bound(&bindings::rows::SUBMIT_REVIEW),
    Entry::Bound(&bindings::rows::START_REVIEW),
    Entry::Bound(&bindings::rows::DISCARD_REVIEW),
    Entry::Bound(&bindings::rows::MERGE),
    Entry::Bound(&bindings::rows::REOPEN),
    Entry::Bound(&bindings::rows::RERUN_BUILDS),
    Entry::Bound(&bindings::rows::COMMENT),
    Entry::Bound(&bindings::rows::REPLY),
    Key("^j/^k", "step comment", Needs::Nothing),
    Entry::Bound(&bindings::rows::FILTER_COMMENTS),
    Entry::Bound(&bindings::rows::EDIT_COMMENT),
    Entry::Delete,
    Entry::Bound(&bindings::rows::RESOLVE_THREAD),
    Entry::Bound(&bindings::rows::REFRESH),
    Key("?", "toggle help", Needs::Nothing),
    Key("q", "quit", Needs::Nothing),
];

/// What the help is drawn for: the provider and the PR in front of the reader.
struct Offered<'a> {
    caps: &'a Capabilities,
    pr_link: bool,
    ai_filter: bool,
}

impl Needs {
    fn holds(self, on: &Offered<'_>) -> bool {
        let caps = on.caps;
        match self {
            Self::Nothing => true,
            Self::PrLink => on.pr_link,
            Self::Reviews => caps.reviews(),
            Self::MergeStrategy => !caps.merge_strategies.is_empty(),
            Self::CloseOrReopen => {
                caps.supports(Feature::ClosePr) || caps.supports(Feature::ReopenPr)
            }
            Self::AnyComment => {
                caps.supports(Feature::PrComments)
                    || caps.supports(Feature::InlineComments)
                    || caps.supports(Feature::Replies)
            }
            Self::AiFilter => on.ai_filter,
            Self::Feature(feature) => caps.supports(feature),
        }
    }
}

impl Entry {
    /// The keys where everything is supported, which is what `docs/KEYS.md`
    /// lists.
    #[cfg(test)]
    fn keys(self) -> &'static str {
        match self {
            Self::Key(keys, ..) => keys,
            Self::Bound(binding) => binding.doc.as_ref().map_or("", |doc| doc.keys),
            Self::Tabs => "1-5",
            Self::Delete => "d",
        }
    }

    /// The line as the help shows it, or nothing where it is not offered.
    fn shown(self, on: &Offered<'_>) -> Option<(&'static str, &'static str)> {
        let caps = on.caps;
        match self {
            Self::Key(keys, text, needs) => needs.holds(on).then_some((keys, text)),
            Self::Bound(binding) => {
                let doc = binding.doc.as_ref()?;
                binding.needs.holds(on).then_some((doc.keys, doc.text))
            }
            Self::Tabs => {
                let keys = if caps.supports(Feature::Builds) {
                    "1-5"
                } else {
                    "1-4"
                };
                Some((keys, "select tab"))
            }
            Self::Delete => {
                let text = match (caps.supports(Feature::DeleteComments), caps.reviews()) {
                    (true, true) => "delete own / pending",
                    (true, false) => "delete own",
                    (false, true) => "remove pending comment",
                    (false, false) => return None,
                };
                Some(("d", text))
            }
        }
    }
}

pub(in crate::tui::ui::screens::pr_detail) fn entries(
    caps: &Capabilities,
    has_pr_link: bool,
    has_ai_filter: bool,
) -> Vec<(&'static str, &'static str)> {
    let on = Offered {
        caps,
        pr_link: has_pr_link,
        ai_filter: has_ai_filter,
    };
    HELP.iter().filter_map(|entry| entry.shown(&on)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        capabilities::{ReviewCaps, ReviewSubmission},
        pr::MergeStrategy,
    };

    #[test]
    fn the_help_and_keys_md_name_the_same_keys_for_a_pr() {
        crate::doc_contract::assert_keys_match(HELP.iter().map(|entry| entry.keys()), "A PR");
    }

    fn keys_of(caps: &Capabilities, link: bool, ai: bool) -> Vec<&'static str> {
        entries(caps, link, ai)
            .into_iter()
            .map(|(keys, _)| keys)
            .collect()
    }

    fn with(features: &[Feature], reviews: bool) -> Capabilities {
        Capabilities {
            features: features.iter().copied().collect(),
            review: reviews.then(|| ReviewCaps {
                verdicts: Vec::new(),
                own_pr_verdicts: Vec::new(),
                submission: ReviewSubmission::Sequential,
            }),
            merge_strategies: Vec::new(),
        }
    }

    #[test]
    fn a_provider_with_only_the_reading_flows_is_offered_only_the_reading_keys() {
        assert_eq!(
            keys_of(&Capabilities::default(), false, false),
            [
                "j/k", "^d/^u", "h/l", "H/L", "1-4", "enter", "space", "/", "n/N", "[ ]", "esc",
                "^j/^k", "F", "?", "q"
            ]
        );
    }

    #[test]
    fn each_need_brings_its_keys_and_nothing_else() {
        let reading = keys_of(&Capabilities::default(), false, false);
        let added = |caps: &Capabilities, link, ai| -> Vec<&'static str> {
            keys_of(caps, link, ai)
                .into_iter()
                .filter(|keys| !reading.contains(keys))
                .collect()
        };
        assert_eq!(added(&Capabilities::default(), true, false), ["o", "y"]);
        assert_eq!(added(&Capabilities::default(), false, true), ["f"]);
        assert_eq!(added(&with(&[], true), false, false), ["a", "v", "V", "d"]);
        assert_eq!(
            added(&with(&[Feature::Builds], false), false, false),
            ["1-5"]
        );
        assert_eq!(
            added(&with(&[Feature::ReopenPr], false), false, false),
            ["x"]
        );
        assert_eq!(
            added(&with(&[Feature::InlineComments], false), false, false),
            ["c"]
        );
        assert_eq!(
            added(&with(&[Feature::Replies], false), false, false),
            ["c", "r"]
        );
        assert_eq!(
            added(&with(&[Feature::EditComments], false), false, false),
            ["e"]
        );
        assert_eq!(
            added(&with(&[Feature::ResolveThreads], false), false, false),
            ["R"]
        );
        let merging = Capabilities {
            merge_strategies: vec![MergeStrategy::Squash],
            ..Capabilities::default()
        };
        assert_eq!(added(&merging, false, false), ["m"]);
    }

    #[test]
    fn the_pr_actions_name_no_single_tab_as_they_work_on_the_description_too() {
        let caps = Capabilities {
            merge_strategies: vec![MergeStrategy::Squash],
            ..with(&[Feature::ReopenPr], true)
        };
        for key in ["a", "m", "x"] {
            let text = entries(&caps, false, false)
                .into_iter()
                .find(|(keys, _)| *keys == key)
                .map(|(_, text)| text);
            assert!(
                text.is_some_and(|text| !text.contains("Overview")),
                "{key}: {text:?}"
            );
        }
    }

    #[test]
    fn d_says_what_it_can_remove() {
        let text = |caps: &Capabilities| {
            entries(caps, false, false)
                .into_iter()
                .find(|(keys, _)| *keys == "d")
                .map(|(_, text)| text)
        };
        assert_eq!(
            text(&with(&[Feature::DeleteComments], false)),
            Some("delete own")
        );
        assert_eq!(
            text(&with(&[Feature::DeleteComments], true)),
            Some("delete own / pending")
        );
        assert_eq!(text(&with(&[], true)), Some("remove pending comment"));
        assert_eq!(text(&with(&[], false)), None);
    }
}
