use crate::domain::{
    activity::Activity,
    ci::Build,
    commit::Commit,
    diff::Diff,
    pr::{Mergeability, PullRequest},
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct Store {
    pub refresh_failures: HashSet<FetchKey>,
    pub link_pending: bool,
    pub notice: Option<Notice>,
    pub draft_error: Option<String>,
    pub uncertain_submissions: std::collections::BTreeSet<u64>,
    pub operations: HashMap<u64, Operation>,
    pub errors: HashMap<u64, String>,
    pub fetches: HashSet<FetchKey>,
    pub reload_after_fetch: HashSet<FetchKey>,
    pub reviews: HashMap<u64, crate::app::reviews::PendingReview>,
    pub cache: Cache,
    pub current_user: String,
    pub capabilities: crate::domain::capabilities::Capabilities,
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
            || self.mergeability.is_loading()
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

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Comment,
    Moderation,
    Review,
    Merge,
    Decline,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchKey {
    Prs,
    Commits(u64),
    Diff(u64),
    Builds(u64),
    Activity(u64),
    Mergeability(u64),
    CommitDiff(u64, String),
}

impl Store {
    pub fn refreshing(&self, screen: crate::app::navigation::Screen) -> bool {
        use crate::app::navigation::Screen;
        self.fetches.iter().any(|key| match (screen, key) {
            (_, FetchKey::Prs) => true,
            (
                Screen::Detail { pr_id, .. },
                FetchKey::Commits(id)
                | FetchKey::Diff(id)
                | FetchKey::Builds(id)
                | FetchKey::Activity(id)
                | FetchKey::Mergeability(id)
                | FetchKey::CommitDiff(id, _),
            ) => pr_id == *id,
            _ => false,
        })
    }
}

#[derive(Debug)]
pub struct Notice {
    pub message: String,
    pub error: bool,
    created: std::time::Instant,
}
impl Notice {
    pub fn new(message: String, error: bool) -> Self {
        Self {
            message,
            error,
            created: std::time::Instant::now(),
        }
    }
    pub fn visible(&self) -> bool {
        self.created.elapsed() < std::time::Duration::from_secs(if self.error { 5 } else { 2 })
    }
}

impl Store {
    pub fn has_cached_data(&self, key: &FetchKey) -> bool {
        let id = match key {
            FetchKey::Prs => return matches!(self.cache.prs, LoadState::Loaded(_)),
            FetchKey::Commits(id)
            | FetchKey::Diff(id)
            | FetchKey::Builds(id)
            | FetchKey::Activity(id)
            | FetchKey::Mergeability(id)
            | FetchKey::CommitDiff(id, _) => id,
        };
        let Some(data) = self.cache.details.get(id) else {
            return false;
        };
        match key {
            FetchKey::Prs => false,
            FetchKey::Commits(_) => matches!(data.commits, LoadState::Loaded(_)),
            FetchKey::Diff(_) => matches!(data.diff, LoadState::Loaded(_)),
            FetchKey::Builds(_) => matches!(data.builds, LoadState::Loaded(_)),
            FetchKey::Activity(_) => matches!(data.activity, LoadState::Loaded(_)),
            FetchKey::Mergeability(_) => matches!(data.mergeability, LoadState::Loaded(_)),
            FetchKey::CommitDiff(_, oid) => {
                matches!(data.commit_diffs.get(oid), Some(LoadState::Loaded(_)))
            }
        }
    }
    pub fn refresh_failed(&self, screen: crate::app::navigation::Screen) -> bool {
        use crate::{app::navigation::Screen, tui::screens::pr_detail::tabs::DetailTab};
        self.refresh_failures.iter().any(|key| match (screen, key) {
            (_, FetchKey::Prs) => true,
            (Screen::Detail { pr_id, tab }, FetchKey::Diff(id)) => {
                pr_id == *id && tab == DetailTab::Diff
            }
            (Screen::Detail { pr_id, tab }, FetchKey::Activity(id)) => {
                pr_id == *id
                    && matches!(
                        tab,
                        DetailTab::Overview | DetailTab::Diff | DetailTab::Commits
                    )
            }
            (Screen::Detail { pr_id, tab }, FetchKey::Builds(id)) => {
                pr_id == *id && matches!(tab, DetailTab::Overview | DetailTab::Builds)
            }
            (Screen::Detail { pr_id, .. }, FetchKey::Mergeability(id)) => pr_id == *id,
            (
                Screen::Detail { pr_id, tab },
                FetchKey::Commits(id) | FetchKey::CommitDiff(id, _),
            ) => pr_id == *id && tab == DetailTab::Commits,
            _ => false,
        })
    }
}
