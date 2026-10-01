use crate::domain::diff::Diff;
use crate::providers::error::FetchError;
use crate::providers::github::cli::run_gh;
use crate::providers::unified_diff;

pub fn fetch_diff(pr_number: u64) -> Result<Diff, FetchError> {
    let revision = super::comments::diff_revision(pr_number)?;
    let pr_arg = pr_number.to_string();
    let stdout = run_gh(&["pr", "diff", &pr_arg])?;
    let text = String::from_utf8_lossy(&stdout);
    if revision != super::comments::diff_revision(pr_number)? {
        return Err(FetchError::Stale(
            "The PR changed while loading its diff. Refresh and try again.".into(),
        ));
    }
    let mut diff = unified_diff::parse(&text);
    diff.revision = Some(revision);
    Ok(diff)
}

pub fn fetch_commit_diff(oid: &str) -> Result<Diff, FetchError> {
    let endpoint = format!("repos/{{owner}}/{{repo}}/commits/{oid}");
    let stdout = run_gh(&[
        "api",
        &endpoint,
        "-H",
        "Accept: application/vnd.github.diff",
    ])?;
    let text = String::from_utf8_lossy(&stdout);
    let mut diff = unified_diff::parse(&text);
    diff.revision = Some(crate::domain::diff::DiffRevision {
        head: oid.into(),
        base: None,
        commit: true,
    });
    Ok(diff)
}
