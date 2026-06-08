use super::{ci::CiStatus, repo::Repo, review::Reviewer, user::User};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type BuildStatus = CiStatus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub author: User,
    pub repo: Repo,
    pub ci: CiStatus,
    pub status: PrStatus,
    pub reviewers: Vec<Reviewer>,
    pub labels: Vec<String>,
    pub build_status: Option<BuildStatus>,
    pub comment_count: u32,
    pub source_branch: String,
    pub target_branch: String,
    pub files_changed: Vec<String>,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}
