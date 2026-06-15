use crate::domain::ci::Build;
use crate::providers::error::FetchError;

#[expect(
    clippy::unnecessary_wraps,
    reason = "signature mirrors the other provider"
)]
pub fn fetch_builds(_pr_number: u64) -> Result<Vec<Build>, FetchError> {
    Ok(Vec::new())
}
