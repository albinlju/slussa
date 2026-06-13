use crate::clients::error::FetchError;
use crate::domain::ci::Build;

#[expect(clippy::unnecessary_wraps, reason = "signature mirrors the other backend")]
pub fn fetch_builds(_pr_number: u64) -> Result<Vec<Build>, FetchError> {
    Ok(Vec::new())
}
