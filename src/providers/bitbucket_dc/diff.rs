use super::{Config, json_diff};
use crate::domain::diff::Diff;
use crate::domain::pr::PrId;
use crate::providers::error::FetchError;

pub fn fetch_diff(config: &Config, pr_id: PrId) -> Result<Diff, FetchError> {
    json_diff::fetch(config, pr_id)
}

pub fn fetch_commit_diff(config: &Config, oid: &str) -> Result<Diff, FetchError> {
    json_diff::fetch_commit(config, oid)
}
