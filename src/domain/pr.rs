use super::{ci::CiSummary, review::Reviewer, user::User};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum PrStatus {
    Open,
    Draft,
    Merged,
    Declined,
}

impl PrStatus {
    pub fn label(&self) -> &str {
        match self {
            PrStatus::Open => "Open",
            PrStatus::Draft => "Draft",
            PrStatus::Merged => "Merged",
            PrStatus::Declined => "Declined",
        }
    }
}

/// Whether a PR can be merged into its target. Coarse on purpose — provider
/// specifics (behind target, blocked on approvals) collapse to `Unknown` for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mergeability {
    Mergeable,
    Conflicts,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub author: User,
    pub ci: CiSummary,
    pub status: PrStatus,
    pub reviewers: Vec<Reviewer>,
    pub labels: Vec<String>,
    pub comment_count: u32,
    pub source_branch: String,
    pub target_branch: String,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}
