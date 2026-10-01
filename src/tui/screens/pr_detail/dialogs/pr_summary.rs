use crate::domain::pr::PullRequest;

/// The PR a dialog is asked about, as the dialog names it. A dialog is given
/// this when it is drawn, so it never holds a copy that could go out of date.
#[derive(Debug, Clone)]
pub struct PrSummary<'a> {
    pub label: String,
    pub target_branch: &'a str,
    pub source_branch: &'a str,
}

impl<'a> PrSummary<'a> {
    pub fn of(pr: &'a PullRequest) -> Self {
        Self {
            label: format!("PR #{} · {}", pr.id, pr.title),
            target_branch: &pr.target_branch,
            source_branch: &pr.source_branch,
        }
    }
}
