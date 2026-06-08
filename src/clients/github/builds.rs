use crate::clients::error::FetchError;
use crate::domain::ci::Build;

pub fn fetch_builds(_pr_number: u64) -> Result<Vec<Build>, FetchError> {
    Ok(Vec::new())
}
