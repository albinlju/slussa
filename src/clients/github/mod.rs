mod activities;
mod builds;
mod cli;
mod comments;
mod commits;
mod diff;
mod events;
mod prs;
mod review_threads;

pub use activities::fetch as fetch_activity;
pub use builds::fetch_builds;
pub use commits::fetch_commits;
pub use diff::{fetch_commit_diff, fetch_diff};
pub use prs::fetch_prs;

/// Map a GitHub reaction name → emoji. Handles both vocabularies: the GraphQL
/// `reactionGroups` content (`THUMBS_UP`) used by `pr view`, and the REST
/// `reactions` keys (`+1`) used by the review-comments API. `None` for unknown.
pub(super) fn reaction_emoji(name: &str) -> Option<&'static str> {
    Some(match name {
        "THUMBS_UP" | "+1" => "👍",
        "THUMBS_DOWN" | "-1" => "👎",
        "LAUGH" | "laugh" => "😄",
        "HOORAY" | "hooray" => "🎉",
        "CONFUSED" | "confused" => "😕",
        "HEART" | "heart" => "❤",
        "ROCKET" | "rocket" => "🚀",
        "EYES" | "eyes" => "👀",
        _ => return None,
    })
}
