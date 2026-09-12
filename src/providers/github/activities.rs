use super::{comments, events, review_threads};
use crate::domain::activity::Activity;
use crate::providers::error::FetchError;

pub fn fetch(pr_number: u64) -> Result<Activity, FetchError> {
    let comments = comments::fetch_comments(pr_number)?;
    Ok(Activity {
        comments,
        events: events::fetch_events(pr_number)?,
        threads: review_threads::fetch_review_threads(pr_number)?,
    })
}
