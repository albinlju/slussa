//! Merging a PR: now, by itself when ready, and deleting its branch after.

use super::cli;
use crate::domain::pr::{DeletableBranch, MergeStrategy, PrId};
use crate::providers::error::{FetchError, MergeError};

/// Merge the PR, then delete the branch it came from when asked to. A branch
/// that cannot be deleted does not undo the merge: that is `BranchDeleteFailed`.
pub fn merge(
    pr_number: PrId,
    strategy: MergeStrategy,
    delete: Option<&DeletableBranch>,
) -> Result<(), MergeError> {
    let method = match strategy {
        MergeStrategy::Merge => "merge",
        MergeStrategy::Squash => "squash",
        MergeStrategy::Rebase => "rebase",
    };
    cli::run_gh(&[
        "api",
        "--method",
        "PUT",
        &format!("repos/{{owner}}/{{repo}}/pulls/{pr_number}/merge"),
        "-f",
        &format!("merge_method={method}"),
    ])?;
    if let Some(branch) = delete {
        delete_branch(branch).map_err(MergeError::BranchDeleteFailed)?;
    }
    Ok(())
}

/// A branch that is already gone counts as deleted: GitHub removes the head
/// branch itself where the repository asks it to, and answers 422 `Reference
/// does not exist` to a delete that comes second.
fn delete_branch(branch: &DeletableBranch) -> Result<(), FetchError> {
    let deleted = cli::run_gh(&[
        "api",
        "--method",
        "DELETE",
        &format!(
            "repos/{{owner}}/{{repo}}/git/refs/heads/{}",
            ref_path(branch.name())
        ),
    ]);
    match deleted {
        Ok(_) => Ok(()),
        Err(FetchError::GhFailed { ref stderr, .. })
            if stderr.contains("Reference does not exist") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// A branch name as the tail of a URL path: slashes stay, anything that is not
/// plain is escaped, so a `#` or `?` in a name does not end the path.
fn ref_path(name: &str) -> String {
    use std::fmt::Write;
    name.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
        out
    })
}

/// Merge the PR by itself with `strategy` once its checks and reviews allow it,
/// or stop it from doing so (`None`). GitHub refuses when the repository does
/// not allow auto-merge; that message reaches the user as it is.
pub fn set_auto_merge(pr_number: PrId, strategy: Option<MergeStrategy>) -> Result<(), FetchError> {
    let pr = pr_number.to_string();
    let flag = match strategy {
        Some(MergeStrategy::Merge) => "--merge",
        Some(MergeStrategy::Squash) => "--squash",
        Some(MergeStrategy::Rebase) => "--rebase",
        None => "--disable-auto",
    };
    let args: &[&str] = if strategy.is_some() {
        &["pr", "merge", &pr, "--auto", flag]
    } else {
        &["pr", "merge", &pr, flag]
    };
    cli::run_gh(args)?;
    Ok(())
}
