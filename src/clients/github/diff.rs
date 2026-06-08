use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh;
use crate::clients::unified_diff;
use crate::domain::diff::Diff;

pub fn fetch_diff(pr_number: u64) -> Result<Diff, FetchError> {
    let pr_arg = pr_number.to_string();
    let stdout = run_gh(&["pr", "diff", &pr_arg])?;
    let text = String::from_utf8_lossy(&stdout);
    Ok(unified_diff::parse(&text))
}
