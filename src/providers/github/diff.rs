use super::GhRepo;
use crate::domain::pr::PrId;
use crate::domain::{
    commit::CommitOid,
    diff::{Diff, DiffRange},
};
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

/// What changed from `range.base` to `range.head`, as GitHub compares them: from
/// their common ancestor, so after a merge of the target into the branch it holds
/// what the merge brought too. A base that was force-pushed away is not there to
/// compare, and GitHub answers 404; that is told as what it most likely is, with
/// what GitHub said.
pub fn fetch_range_diff(repo: &GhRepo, range: &DiffRange) -> Result<Diff, FetchError> {
    let endpoint = format!(
        "repos/{{owner}}/{{repo}}/compare/{}...{}",
        range.base, range.head
    );
    let stdout = run_gh(
        repo,
        &[
            "api",
            &endpoint,
            "-H",
            "Accept: application/vnd.github.diff",
        ],
    )
    .map_err(|error| {
        if let FetchError::GhFailed { stderr, .. } = &error
            && stderr.contains("HTTP 404")
        {
            return FetchError::Stale(format!(
                "The commit you read is probably no longer on GitHub, since the branch was \
                 force-pushed, so what is new cannot be told (GitHub said: {stderr}). \
                 w: the whole diff."
            ));
        }
        error
    })?;
    let text = String::from_utf8_lossy(&stdout);
    let mut diff = unified_diff::parse(&text);
    diff.revision = Some(crate::domain::diff::DiffRevision {
        head: range.head.as_str().into(),
        base: Some(range.base.as_str().into()),
        commit: true,
    });
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
