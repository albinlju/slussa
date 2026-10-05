//! Asking people to review a PR.

use super::GhRepo;
use super::cli;
use crate::domain::{pr::PrId, review::Rerequest};
use crate::providers::error::FetchError;

/// Ask those who asked for changes to review again. GitHub refuses an account
/// it cannot request, such as a bot; that message reaches the user as it is.
pub fn rerequest(repo: &GhRepo, pr_number: PrId, who: &Rerequest) -> Result<(), FetchError> {
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/requested_reviewers");
    let fields: Vec<String> = who
        .names()
        .iter()
        .map(|name| format!("reviewers[]={name}"))
        .collect();
    let mut args = vec!["api", "--method", "POST", endpoint.as_str()];
    for field in &fields {
        args.push("-f");
        args.push(field);
    }
    cli::run_gh(repo, &args)?;
    Ok(())
}
