use super::GhRepo;
use crate::{
    domain::pr::{IssueText, LinkedIssue},
    providers::FetchError,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Issue {
    title: String,
    body: Option<String>,
}

/// What an issue the PR closes says. An issue in another repository is not asked
/// for: its number means nothing here, and the call would read another issue.
pub fn fetch_issue_text(
    repo: &GhRepo,
    issue: &LinkedIssue,
) -> Result<Option<IssueText>, FetchError> {
    let here = format!("/{}/issues/{}", repo.slug(), issue.number).to_ascii_lowercase();
    let in_this_repo = issue
        .url
        .as_deref()
        .is_some_and(|url| url.to_ascii_lowercase().ends_with(&here));
    if !in_this_repo {
        return Ok(None);
    }
    let read: Issue = super::cli::run_gh_json(
        repo,
        &[
            "api",
            &format!("repos/{{owner}}/{{repo}}/issues/{}", issue.number),
        ],
    )?;
    Ok(Some(IssueText {
        number: issue.number,
        title: read.title,
        body: read.body.filter(|body| !body.trim().is_empty()),
    }))
}
