use crate::domain::capabilities::{Capabilities, Feature};

const HELP_KEYS: &[(&str, &str)] = &[
    ("o", "open PR in browser"),
    ("y", "copy PR link"),
    ("j/k", "move up/down"),
    ("^d/^u", "half-page"),
    ("h/l", "tab / pane / fold"),
    ("H/L", "pan wide Description"),
    ("1-5", "select tab"),
    ("enter", "open / view"),
    ("space", "toggle fold"),
    ("/", "search"),
    ("n/N", "next/prev match"),
    ("[ ]", "prev/next tab/commit"),
    ("esc", "back"),
    ("a", "submit review (Overview)"),
    ("v", "start/finish review draft"),
    ("V", "discard review"),
    ("m", "merge (Overview)"),
    ("x", "close / decline (Overview)"),
    ("c", "comment"),
    ("r", "reply"),
    ("^j/^k", "step comment"),
    ("e", "edit own"),
    ("d", "delete own"),
    ("R", "resolve thread"),
    ("F", "refresh"),
    ("?", "toggle help"),
    ("q", "quit"),
];

pub(in crate::tui::screens::pr_detail) fn entries(
    caps: &Capabilities,
    has_pr_link: bool,
) -> Vec<(&'static str, &'static str)> {
    let keys: Vec<_> = HELP_KEYS
        .iter()
        .copied()
        .filter(|(key, _)| match *key {
            "o" | "y" => has_pr_link,
            "a" | "v" | "V" => caps.reviews(),
            "m" => !caps.merge_strategies.is_empty(),
            "x" => caps.supports(Feature::ClosePr),
            "c" => {
                caps.supports(Feature::PrComments)
                    || caps.supports(Feature::InlineComments)
                    || caps.supports(Feature::Replies)
            }
            "r" => caps.supports(Feature::Replies),
            "e" => caps.supports(Feature::EditComments),
            "d" => caps.supports(Feature::DeleteComments) || caps.reviews(),
            "R" => caps.supports(Feature::ResolveThreads),
            _ => true,
        })
        .map(|(key, desc)| {
            let desc = if key == "d" && caps.reviews() {
                if caps.supports(Feature::DeleteComments) {
                    "delete own / pending"
                } else {
                    "remove pending comment"
                }
            } else {
                desc
            };
            (
                if key == "1-5" && !caps.supports(Feature::Builds) {
                    "1-4"
                } else {
                    key
                },
                desc,
            )
        })
        .collect();
    keys
}
