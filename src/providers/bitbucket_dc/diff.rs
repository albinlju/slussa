use super::{Config, structured};
use crate::providers::error::FetchError;
use crate::domain::diff::Diff;

pub fn fetch_diff(config: &Config, pr_id: u64) -> Result<Diff, FetchError> {
    structured::fetch(config, pr_id)
}

pub fn fetch_commit_diff(config: &Config, oid: &str) -> Result<Diff, FetchError> {
    structured::fetch_commit(config, oid)
}
