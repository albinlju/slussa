//! Inline review threads (the comments on the diff). Bitbucket DC delivers
//! these inside the structured `/diff?withComments=true` payload, anchored to
//! the exact line each comment sits on. We share that single fetch with the
//! Diff tab via [`super::structured`] and return only the threads here.

use super::{Config, structured};
use crate::clients::error::FetchError;
use crate::domain::comment::ReviewThread;

pub fn fetch_review_threads(
    config: &Config,
    pr_id: u64,
) -> Result<Vec<ReviewThread>, FetchError> {
    Ok(structured::fetch(config, pr_id)?.threads)
}
