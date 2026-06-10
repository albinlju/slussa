/// Rollup CI state for the PR-list badge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CiState {
    Pending,
    Success,
    Failed,
    Unknown,
}

/// State of a single build/check, mirroring Bitbucket DC's build-status
/// states. Kept separate from [`CiState`] (the PR-list rollup badge) because
/// the Builds tab needs to distinguish in-progress and cancelled builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildState {
    Successful,
    Failed,
    InProgress,
    Cancelled,
    Unknown,
}

/// One build/check reported against the PR's source commit.
#[derive(Debug, Clone)]
pub struct Build {
    pub name: String,
    pub state: BuildState,
    /// Wall-clock build time in milliseconds, when the provider reports it.
    pub duration_ms: Option<u64>,
}
