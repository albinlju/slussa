use super::GhRepo;
use crate::domain::pr::PrId;
use crate::domain::{
    commit::CommitOid,
    diff::{Compared, Diff, DiffRange},
};
use crate::providers::error::FetchError;
use crate::providers::github::cli::run_gh;
use crate::providers::unified_diff;
use std::collections::HashSet;

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

/// As many files as GitHub lists for a compare; with that many, there may be more.
const COMPARE_FILES: usize = 300;

/// What changed from `range.base` to `range.head`, as GitHub compares them: from
/// their common ancestor, so after a merge of the target into the branch it holds
/// what the merge brought too. The diff's base is that ancestor, the commit the
/// compare really starts from, which says whether the branch moved forward, was
/// reset or was rewritten (`DiffRange::moved`). With `target`, the commit the PR
/// is against, the files the PR touched at `range.base` are listed too, so that
/// the caller can tell a file the merge brought from one the PR put back. A base
/// that was force-pushed away is not there to compare, and GitHub answers 404;
/// that is told as what it most likely is, with what GitHub said.
pub fn fetch_range_diff(
    repo: &GhRepo,
    range: &DiffRange,
    target: Option<&CommitOid>,
) -> Result<Compared, FetchError> {
    #[derive(serde::Deserialize)]
    struct Commit {
        sha: String,
    }
    #[derive(serde::Deserialize)]
    struct About {
        merge_base_commit: Commit,
    }
    let endpoint = format!(
        "repos/{{owner}}/{{repo}}/compare/{}...{}",
        range.base, range.head
    );
    // Only the first page of a compare lists the files; a later one holds what
    // is asked for here and little else.
    let about = format!("{endpoint}?per_page=1&page=2");
    let about: About = super::cli::run_gh_json(repo, &["api", &about]).map_err(commit_gone)?;
    let stdout = run_gh(
        repo,
        &[
            "api",
            &endpoint,
            "-H",
            "Accept: application/vnd.github.diff",
        ],
    )
    .map_err(commit_gone)?;
    let text = String::from_utf8_lossy(&stdout);
    let mut diff = unified_diff::parse(&text);
    diff.revision = Some(crate::domain::diff::DiffRevision {
        head: range.head.as_str().into(),
        base: Some(about.merge_base_commit.sha),
        commit: true,
    });
    let in_pr_before = match target {
        Some(target) => files_changed(repo, target, &range.base)?,
        None => None,
    };
    Ok(Compared { diff, in_pr_before })
}

/// The files `commit` changed since it left `target`: what a PR against `target`
/// touched when its branch was at `commit`. `None` when GitHub lists as many as it
/// lists at most, since there may then be more.
fn files_changed(
    repo: &GhRepo,
    target: &CommitOid,
    commit: &CommitOid,
) -> Result<Option<HashSet<String>>, FetchError> {
    #[derive(serde::Deserialize)]
    struct File {
        filename: String,
        previous_filename: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Changed {
        #[serde(default)]
        files: Vec<File>,
    }
    let endpoint = format!("repos/{{owner}}/{{repo}}/compare/{target}...{commit}");
    let changed: Changed =
        super::cli::run_gh_json(repo, &["api", &endpoint]).map_err(commit_gone)?;
    if changed.files.len() >= COMPARE_FILES {
        return Ok(None);
    }
    Ok(Some(
        changed
            .files
            .into_iter()
            .flat_map(|file| [Some(file.filename), file.previous_filename])
            .flatten()
            .collect(),
    ))
}

/// A compare that GitHub answers with 404 is most likely of a commit that is gone.
fn commit_gone(error: FetchError) -> FetchError {
    if let FetchError::GhFailed { stderr, .. } = &error
        && stderr.contains("HTTP 404")
    {
        return FetchError::Stale(format!(
            "The commit you read is probably no longer on GitHub, since the branch was \
             force-pushed, so what is new cannot be told (GitHub said: {stderr})."
        ));
    }
    error
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
