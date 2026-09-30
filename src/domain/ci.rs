#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiSummary {
    Pending,
    Success,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildState {
    Successful,
    Failed,
    InProgress,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Build {
    pub name: String,
    pub state: BuildState,
    pub duration_ms: Option<u64>,
}
