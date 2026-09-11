use crate::domain::{
    activity::Activity,
    ci::Build,
    commit::Commit,
    diff::Diff,
    pr::{MergeStrategy, Mergeability, PullRequest},
};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Store {
    pub reviews: HashMap<u64, crate::app::reviews::PendingReview>,
    pub cache: Cache,
    pub current_user: String,
    pub can_unapprove: bool,
    pub merge_strategies: Vec<MergeStrategy>,
}

#[derive(Debug, Default)]
pub struct Cache {
    pub prs: LoadState<Vec<PullRequest>>,
    pub details: HashMap<u64, PrData>,
}

#[derive(Debug, Default)]
pub struct PrData {
    pub commits: LoadState<Vec<Commit>>,
    pub diff: LoadState<Diff>,
    pub builds: LoadState<Vec<Build>>,
    pub activity: LoadState<Activity>,
    pub mergeability: LoadState<Mergeability>,
    pub commit_diffs: HashMap<String, LoadState<Diff>>,
}

#[derive(Debug, Default)]
pub enum LoadState<T> {
    #[default]
    NotRequested,
    Loading,
    Loaded(T),
    Failed(String),
}

impl<T> LoadState<T> {
    pub fn start_loading(&mut self) -> bool {
        if matches!(self, LoadState::NotRequested | LoadState::Failed(_)) {
            *self = LoadState::Loading;
            true
        } else {
            false
        }
    }

    pub fn from_result(result: Result<T, String>) -> Self {
        match result {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(e),
        }
    }

    /// Apply a (re)fetch result. A failed reload never downgrades data that's
    /// already loaded — a transient refresh error keeps the stale view rather
    /// than blanking it.
    pub fn reload(&mut self, result: Result<T, String>) {
        match result {
            Ok(v) => *self = LoadState::Loaded(v),
            Err(e) => {
                if !matches!(self, LoadState::Loaded(_)) {
                    *self = LoadState::Failed(e);
                }
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, LoadState::Loading)
    }
}

impl PrData {
    pub(crate) fn any_loading(&self) -> bool {
        self.commits.is_loading()
            || self.diff.is_loading()
            || self.builds.is_loading()
            || self.activity.is_loading()
            || self.commit_diffs.values().any(LoadState::is_loading)
    }
}

impl PrData {
    pub fn diff_for(&self, commit: Option<&str>) -> Option<&LoadState<Diff>> {
        match commit {
            Some(oid) => self.commit_diffs.get(oid),
            None => Some(&self.diff),
        }
    }
}
