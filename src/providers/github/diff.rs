use super::GhRepo;
use crate::domain::pr::PrId;
use crate::domain::{commit::CommitOid, diff::Diff};
use crate::providers::error::FetchError;
use crate::providers::github::cli::run_gh;
use crate::providers::unified_diff;

pub fn fetch_diff(repo: &GhRepo, pr_number: PrId) -> Result<Diff, FetchError> {
    let revision = super::comments::diff_revision(repo, pr_number)?;
    let pr_arg = pr_number.to_string();
    let stdout = run_gh(repo, &["pr", "diff", &pr_arg])?;
    let text = String::from_utf8_lossy(&stdout);
    if revision != super::comments::diff_revision(repo, pr_number)? {
        return Err(FetchError::Stale(
            "The PR changed while loading its diff. Refresh and try again.".into(),
        ));
    }
    let mut diff = unified_diff::parse(&text);
    diff.revision = Some(revision);
    Ok(diff)
}

pub fn fetch_commit_diff(repo: &GhRepo, oid: &CommitOid) -> Result<Diff, FetchError> {
    let endpoint = format!("repos/{{owner}}/{{repo}}/commits/{oid}");
    let stdout = run_gh(
        repo,
        &[
            "api",
            &endpoint,
            "-H",
            "Accept: application/vnd.github.diff",
        ],
    )?;
    let text = String::from_utf8_lossy(&stdout);
    let mut diff = unified_diff::parse(&text);
    diff.revision = Some(crate::domain::diff::DiffRevision {
        head: oid.as_str().into(),
        base: None,
        commit: true,
    });
    Ok(diff)
}
