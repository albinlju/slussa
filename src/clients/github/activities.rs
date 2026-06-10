//! Bundles GitHub's three activity sources into one [`ActivityBundle`].
//! Comments are the primary content, so their failure fails the load; events
//! and threads degrade to empty with a warning rather than blanking the
//! conversation.

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
