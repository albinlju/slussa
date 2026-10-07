//! What a review is made from besides its instructions: the PR as its provider
//! tells it, the diff of its head, the issues it closes and the repository's own
//! rules. Read the same way for the agent slussa asks (`A`) and for the agent
//! that asks slussa (`slussa context`), so that both review the same thing.

use std::path::Path;

use super::prompt::{ReviewRequest, RulesFile};
use crate::{
    domain::{
        commit::CommitOid,
        diff::Diff,
        pr::{IssueText, LinkedIssue, PrId},
    },
    providers::{FetchError, Provider},
};

/// The files where a repository writes down its own rules.
const RULES_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md"];

/// What the PR is, as the agent is told it.
pub struct Subject {
    pub title: String,
    pub description: Option<String>,
    pub source_branch: String,
    pub target_branch: String,
    /// The issues the PR closes, as the provider listed them.
    pub issues: Vec<LinkedIssue>,
}

impl Subject {
    /// Read from the provider, for a caller with no list to take it from. A
    /// provider that lists the description with the PR has nothing more to say.
    pub fn read(provider: &Provider, pr_id: PrId) -> Result<Self, FetchError> {
        let pr = provider.fetch_pr(pr_id)?;
        let info = match provider.fetch_info(pr_id) {
            Ok(info) => Some(info),
            Err(FetchError::Unsupported(_)) => None,
            Err(error) => return Err(error),
        };
        let (described, issues) =
            info.map_or((None, Vec::new()), |info| (info.description, info.issues));
        Ok(Self {
            title: pr.title,
            description: described.or(pr.description),
            source_branch: pr.source_branch,
            target_branch: pr.target_branch,
            issues,
        })
    }
}

/// The diff of the PR's head, and what was read to go with it.
pub struct Material {
    /// The commit the diff is of: what a proposal made from it is tied to.
    pub head: CommitOid,
    pub diff_text: String,
    pub diff: Diff,
    pub issues: Vec<IssueText>,
    /// Issues the PR closes that could not be read.
    pub issues_missed: usize,
    pub rules: Vec<RulesFile>,
}

#[derive(Debug, thiserror::Error)]
pub enum MaterialError {
    #[error(transparent)]
    Provider(#[from] FetchError),
    #[error("The PR's head commit is not known.")]
    NoHead,
}

impl Material {
    /// Read the diff, the issues `subject` names and the rules under `root`.
    pub fn gather(
        provider: &Provider,
        pr_id: PrId,
        subject: &Subject,
        root: Option<&Path>,
    ) -> Result<Self, MaterialError> {
        let (diff_text, revision) = provider.fetch_diff_text(pr_id)?;
        let head = CommitOid::parse(&revision.head).ok_or(MaterialError::NoHead)?;
        let diff = crate::providers::parse_unified_diff(&diff_text);
        let (issues, issues_missed) = read_issues(provider, &subject.issues);
        Ok(Self {
            head,
            diff_text,
            diff,
            issues,
            issues_missed,
            rules: read_rules(root),
        })
    }

    /// What the agent is told, from this and the PR it is of.
    pub fn request<'a>(
        &'a self,
        subject: &'a Subject,
        instructions: Option<&'a str>,
    ) -> ReviewRequest<'a> {
        ReviewRequest {
            title: &subject.title,
            description: subject.description.as_deref(),
            source_branch: &subject.source_branch,
            target_branch: &subject.target_branch,
            head: self.head.as_str(),
            diff: &self.diff_text,
            issues: &self.issues,
            rules: &self.rules,
            instructions,
        }
    }
}

/// What the issues the PR closes say, and how many could not be read.
fn read_issues(provider: &Provider, listed: &[LinkedIssue]) -> (Vec<IssueText>, usize) {
    let mut read = Vec::new();
    let mut missed = 0;
    for issue in listed {
        match provider.fetch_issue_text(issue) {
            Ok(Some(text)) => read.push(text),
            // In another repository: nothing here to ask for, and not a failure.
            Ok(None) => {}
            Err(error) => {
                tracing::warn!("could not read issue #{}: {error}", issue.number);
                missed += 1;
            }
        }
    }
    (read, missed)
}

/// The repository's own rules, from the files it keeps them in.
fn read_rules(root: Option<&Path>) -> Vec<RulesFile> {
    let Some(root) = root else {
        return Vec::new();
    };
    RULES_FILES
        .iter()
        .filter_map(|name| {
            let text = std::fs::read_to_string(root.join(name)).ok()?;
            (!text.trim().is_empty()).then(|| RulesFile {
                name: (*name).to_owned(),
                text,
            })
        })
        .collect()
}
