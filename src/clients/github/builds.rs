use crate::clients::error::FetchError;
use crate::domain::ci::Build;

/// Not implemented for GitHub yet — CI surfaces only as the PR list's rollup
/// badge (`prs::summarize_checks`). A per-check Builds tab needs the
/// `statusCheckRollup` details; until then the tab shows its empty state.
pub fn fetch_builds(_pr_number: u64) -> Result<Vec<Build>, FetchError> {
    Ok(Vec::new())
}
