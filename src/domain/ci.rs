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

/// A job of a GitHub Actions workflow run, which is what a build log belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Build {
    pub name: String,
    pub state: BuildState,
    pub duration_ms: Option<u64>,
    /// The job whose log can be read; none for a check that is not an Action.
    pub log: Option<JobId>,
}
