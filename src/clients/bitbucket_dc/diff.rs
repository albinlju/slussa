//! The Diff tab's data comes from Bitbucket DC's structured `/diff` JSON,
//! parsed in [`super::structured`]. We discard the inline comments here and
//! return just the [`Diff`]; `review_threads` reads the same payload for the
//! comment side.

use super::{Config, structured};
use crate::clients::error::FetchError;
use crate::domain::diff::Diff;

pub fn fetch_diff(config: &Config, pr_id: u64) -> Result<Diff, FetchError> {
    Ok(structured::fetch(config, pr_id)?.diff)
}
