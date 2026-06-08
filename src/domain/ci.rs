use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CiState {
    Pending,
    Success,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiStatus {
    pub state: CiState,
    pub description: Option<String>,
    pub url: Option<String>,
}

/// State of a single build/check, mirroring Bitbucket DC's build-status
/// states. Kept separate from [`CiState`] (the PR-list rollup badge) because
/// the Builds tab needs to distinguish in-progress and cancelled builds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BuildState {
    Successful,
    Failed,
    InProgress,
    Cancelled,
    Unknown,
}

/// One build/check reported against the PR's source commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    pub key: String,
    pub name: String,
    pub state: BuildState,
    pub description: Option<String>,
    pub url: Option<String>,
    /// Wall-clock build time in milliseconds, when the provider reports it.
    pub duration_ms: Option<u64>,
}
