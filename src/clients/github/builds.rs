//! GitHub Builds tab. Not wired up yet — `gh pr checks` exits non-zero while
//! checks are pending/failing, so it needs special handling distinct from the
//! other `run_gh` callers. Returning an empty list keeps the tab rendering
//! cleanly until that's built.

use crate::clients::error::FetchError;
use crate::domain::ci::Build;

pub fn fetch_builds(_pr_number: u64) -> Result<Vec<Build>, FetchError> {
    Ok(Vec::new())
}
