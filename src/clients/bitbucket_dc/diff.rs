use super::{Config, structured};
use crate::clients::error::FetchError;
use crate::domain::diff::Diff;

pub fn fetch_diff(config: &Config, pr_id: u64) -> Result<Diff, FetchError> {
    structured::fetch(config, pr_id)
}
