//! Merging a PR: now, by itself when ready, and deleting its branch after.

use super::cli;
use crate::domain::{
    pr::{AutoMerge, DeletableBranch, MergeStrategy, PrId},
    review::ReviewedHead,
};
use crate::providers::error::{FetchError, MergeError};

/// The answer to a merge of a PR whose branch has moved since it was read.
pub(super) fn moved_since_read() -> FetchError {
    FetchError::Stale(
        "The PR changed after you read it. Read what is new in the Diff tab (F refreshes it), then try again.".into(),
    )
}

/// Merge the PR if its branch is still at `head`, then delete the branch it
/// came from when asked to. A branch that cannot be deleted does not undo the
/// merge: that is `BranchDeleteFailed`.
pub fn merge(
    pr_number: PrId,
    strategy: MergeStrategy,
    delete: Option<&DeletableBranch>,
    head: &ReviewedHead,
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
        // GitHub refuses with 409 when the head is not this one, so a push
        // after the PR was read is not merged unseen.
        "-f",
        &format!("sha={}", head.as_str()),
    ])
    .map_err(|error| {
        if let FetchError::GhFailed { stderr, .. } = &error
            && stderr.contains("Head branch was modified")
        {
            moved_since_read()
        } else {
            error
        }
    })?;
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

/// Merge the PR by itself once its checks and reviews allow it, or stop it from
/// doing so. GitHub refuses when the repository does not allow auto-merge; that
/// message reaches the user as it is. Turning it on holds only if the branch is
/// still at the head that was read: a later push is merged once the checks pass.
pub fn set_auto_merge(pr_number: PrId, change: &AutoMerge) -> Result<(), FetchError> {
    let pr = pr_number.to_string();
    match change {
        AutoMerge::On { strategy, head } => {
            let flag = match strategy {
                MergeStrategy::Merge => "--merge",
                MergeStrategy::Squash => "--squash",
                MergeStrategy::Rebase => "--rebase",
            };
            cli::run_gh(&[
                "pr",
                "merge",
                &pr,
                "--auto",
                flag,
                "--match-head-commit",
                head.as_str(),
            ])?;
        }
        AutoMerge::Off => {
            cli::run_gh(&["pr", "merge", &pr, "--disable-auto"])?;
        }
    }
    Ok(())
}
