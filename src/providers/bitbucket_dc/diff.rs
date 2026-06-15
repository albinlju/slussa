use super::{Config, json_diff};
use crate::domain::diff::Diff;
use crate::providers::error::FetchError;

pub fn fetch_diff(config: &Config, pr_id: u64) -> Result<Diff, FetchError> {
    json_diff::fetch(config, pr_id)
}

pub fn fetch_commit_diff(config: &Config, oid: &str) -> Result<Diff, FetchError> {
    json_diff::fetch_commit(config, oid)
}
