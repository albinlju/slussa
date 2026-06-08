//! Bundles GitHub's three activity sources — discussion comments, lifecycle
//! events, and inline review threads — into one [`ActivityBundle`], matching
//! the single-fetch shape the rest of the app consumes. Each piece is a
//! distinct `gh` call (no shared feed like Bitbucket's `/activities`).
//!
//! Comments are the primary content, so a failure there fails the whole load.
//! Events and threads are supplementary: if their `gh` call errors we log it
//! and fall back to empty rather than blanking the conversation.

use super::{comments, events, review_threads};
use crate::clients::ActivityBundle;
use crate::clients::error::FetchError;

pub fn fetch(pr_number: u64) -> Result<ActivityBundle, FetchError> {
    let comments = comments::fetch_comments(pr_number)?;
    Ok(ActivityBundle {
        comments,
        events: events::fetch_events(pr_number).unwrap_or_else(|e| {
            tracing::warn!("github events fetch failed (pr {pr_number}): {e}");
            Vec::new()
        }),
        threads: review_threads::fetch_review_threads(pr_number).unwrap_or_else(|e| {
            tracing::warn!("github review-threads fetch failed (pr {pr_number}): {e}");
            Vec::new()
        }),
    })
}
