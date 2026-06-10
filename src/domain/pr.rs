use super::{ci::CiState, review::Reviewer, user::User};
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

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub author: User,
    pub ci: CiState,
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
