//! Application data shared by every screen: what was fetched, what is in
//! flight and what failed. Components borrow it read-only; only `App` mutates it.
//!
//! - `Cache` holds provider data as `LoadState`s (not requested, loading,
//!   loaded, failed), for the PR list and for each PR's commits, diff, builds,
//!   activity, mergeability and per-commit diffs.
//! - `fetches` holds one `FetchKey` per running read. `begin_fetch` inserts
//!   the key and hands out a `FetchTicket`, which starting a read requires; it
//!   gives none if the key is already there, so a slow older read can never
//!   overwrite a newer one. Completion removes only its own key.
//! - `reload_after_fetch` marks a resource that changed while a read was in
//!   flight; it is fetched again once that read settles.
//! - A failed reload keeps data that is already loaded (`LoadState::reload`)
//!   and records the key in `refresh_failures` so the UI can say so.
//! - `operations` and `errors` are keyed by PR id: one mutation per PR at a
//!   time, and an error appears only on the PR it belongs to. `begin_write`
//!   records the operation and hands out the `WriteTicket` a write requires.
//! - `reviews` holds review drafts by PR id, so a queued review cannot show up
//!   on, or be submitted for, another PR.

use crate::{
    domain::{
        activity::Activity,
        capabilities::{Capabilities, Feature},
        ci::Build,
        commit::Commit,
        diff::Diff,
        pr::{Mergeability, PrGroup, PrInfo, PrStatus, PullRequest},
        user::Username,
    },
    providers::FetchError,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
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
    /// What has been read of each group of PRs.
    pub groups: HashMap<PrGroup, GroupState>,
    pub open_chain: OpenChain,
    /// How many more batches of open PRs `L` has asked for.
    pub open_extra: usize,
    pub current_user: Username,
    pub capabilities: Capabilities,
}

impl Store {
    pub fn new(current_user: Username, capabilities: Capabilities) -> Self {
        Self {
            refresh_failures: HashSet::new(),
            link_pending: false,
            notice: None,
            draft_error: None,
            uncertain_submissions: std::collections::BTreeSet::new(),
            operations: HashMap::new(),
            errors: HashMap::new(),
            fetches: HashSet::new(),
            reload_after_fetch: HashSet::new(),
            reviews: HashMap::new(),
            cache: Cache::default(),
            groups: HashMap::new(),
            open_chain: OpenChain::default(),
            open_extra: 0,
            current_user,
            capabilities,
        }
    }
}

/// A store for a test that does not care who is looking.
#[cfg(test)]
impl Default for Store {
    fn default() -> Self {
        Self::new("viewer".into(), Capabilities::default())
    }
}

/// Where the pages of the open group stand. They are read one after another.
#[derive(Debug, Default)]
pub enum OpenChain {
    #[default]
    Idle,
    /// The first reading: each page is shown as it arrives.
    Appending,
    /// A refresh: the pages are held and swapped in when the last has arrived,
    /// so the list never shrinks to its first page in the meantime.
    Collecting(Vec<PullRequest>),
}

/// What has been read of one group of PRs.
#[derive(Debug, Default, Clone)]
pub struct GroupState {
    pub loaded: bool,
    /// Where to continue reading older PRs of a closed group; `None` when
    /// there are none left.
    pub more: Option<String>,
    /// Whether an older page was read, which a refresh must then keep.
    pub older_loaded: bool,
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
    /// Description and labels, for a provider whose list leaves them out.
    pub info: LoadState<PrInfo>,
    pub commit_diffs: HashMap<String, LoadState<Diff>>,
}

#[derive(Debug, Default)]
pub enum LoadState<T> {
    #[default]
    NotRequested,
    Loading,
    Loaded(T),
    Failed(FetchError),
}

impl<T> LoadState<T> {
    pub fn start_loading(&mut self) -> bool {
        if matches!(self, Self::NotRequested | Self::Failed(_)) {
            *self = Self::Loading;
            true
        } else {
            false
        }
    }

    pub fn from_result(result: Result<T, FetchError>) -> Self {
        match result {
            Ok(v) => Self::Loaded(v),
            Err(e) => Self::Failed(e),
        }
    }

    /// Apply a (re)fetch result. A failed reload never downgrades data that's
    /// already loaded — a transient refresh error keeps the stale view rather
    /// than blanking it.
    pub fn reload(&mut self, result: Result<T, FetchError>) {
        match result {
            Ok(v) => *self = Self::Loaded(v),
            Err(e) => {
                if !matches!(self, Self::Loaded(_)) {
                    *self = Self::Failed(e);
                }
            }
        }
    }

    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// The data, when there is any.
    pub const fn loaded(&self) -> Option<&T> {
        match self {
            Self::Loaded(value) => Some(value),
            Self::NotRequested | Self::Loading | Self::Failed(_) => None,
        }
    }
}

impl PrData {
    pub(crate) fn any_loading(&self) -> bool {
        self.commits.is_loading()
            || self.diff.is_loading()
            || self.builds.is_loading()
            || self.activity.is_loading()
            || self.mergeability.is_loading()
            || self.info.is_loading()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Comment,
    Moderation,
    Review,
    Merge,
    Decline,
    Reopen,
}

impl Operation {
    /// What the notice says once it went through.
    pub const fn done_label(self) -> &'static str {
        match self {
            Self::Merge => "merged",
            Self::Decline => "closed / declined",
            Self::Reopen => "reopened",
            Self::Review => "review submitted",
            Self::Comment => "comment saved",
            Self::Moderation => "comment / thread updated",
        }
    }

    /// The status the PR has once it went through, if it changes it.
    pub const fn moves_pr_to(self) -> Option<PrStatus> {
        match self {
            Self::Merge => Some(PrStatus::Merged),
            Self::Decline => Some(PrStatus::Declined),
            Self::Reopen => Some(PrStatus::Open),
            Self::Comment | Self::Moderation | Self::Review => None,
        }
    }

    /// Whether it sends what is in the comment editor.
    pub const fn sends_editor_text(self) -> bool {
        match self {
            Self::Comment | Self::Review => true,
            Self::Moderation | Self::Merge | Self::Decline | Self::Reopen => false,
        }
    }
}

/// Proof that a read is registered in `Store::fetches`. Only
/// `Store::begin_fetch` makes one, and starting the read takes it, so a read
/// cannot be spawned without being registered first.
#[derive(Debug)]
pub struct FetchTicket(FetchKey);

impl FetchTicket {
    pub const fn key(&self) -> &FetchKey {
        &self.0
    }
}

/// Proof that a write may start: no other was pending for the PR and the
/// operation is recorded. Only `Store::begin_write` makes one, starting the
/// write takes it, and it comes back with the result to say which write ended.
#[derive(Debug)]
pub struct WriteTicket {
    pr_id: u64,
    operation: Operation,
}

impl WriteTicket {
    pub const fn pr_id(&self) -> u64 {
        self.pr_id
    }

    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// The ticket of a write already recorded, for a test that plays its worker.
    #[cfg(test)]
    pub const fn pending(pr_id: u64, operation: Operation) -> Self {
        Self { pr_id, operation }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchKey {
    /// One group of PRs, read first or, for a closed group, one page further.
    Prs(PrGroup),
    Commits(u64),
    Diff(u64),
    Builds(u64),
    Activity(u64),
    Mergeability(u64),
    Info(u64),
    CommitDiff(u64, String),
}

/// How many open PRs are read without being asked to, and how many each `L`
/// adds. Three pages on GitHub. A repository with fewer is read in full.
pub const OPEN_BATCH: usize = 90;

impl FetchKey {
    /// The provider feature this resource needs, if it is an optional one.
    const fn feature(&self) -> Option<Feature> {
        match self {
            Self::Builds(_) => Some(Feature::Builds),
            Self::Mergeability(_) => Some(Feature::Mergeability),
            Self::Info(_) => Some(Feature::PrInfo),
            Self::Prs(_)
            | Self::Commits(_)
            | Self::Diff(_)
            | Self::Activity(_)
            | Self::CommitDiff(..) => None,
        }
    }
}

impl Store {
    /// Whether the provider has this resource at all.
    fn offers(&self, key: &FetchKey) -> bool {
        key.feature()
            .is_none_or(|feature| self.capabilities.supports(feature))
    }

    /// Register a read. `None` when the provider does not have the resource or
    /// a read of it is already running: there is nothing to start then.
    pub fn begin_fetch(&mut self, key: FetchKey) -> Option<FetchTicket> {
        (self.offers(&key) && self.fetches.insert(key.clone())).then_some(FetchTicket(key))
    }

    /// Mark a resource as loading if it has never been read (or failed), and
    /// say whether to read it now. Data already loaded is left as it is.
    pub fn start_loading(&mut self, key: &FetchKey) -> bool {
        if !self.offers(key) {
            return false;
        }
        let id = match key {
            FetchKey::Prs(_) => return self.cache.prs.start_loading(),
            FetchKey::Commits(id)
            | FetchKey::Diff(id)
            | FetchKey::Builds(id)
            | FetchKey::Activity(id)
            | FetchKey::Mergeability(id)
            | FetchKey::Info(id)
            | FetchKey::CommitDiff(id, _) => *id,
        };
        let data = self.cache.details.entry(id).or_default();
        match key {
            FetchKey::Prs(_) => false,
            FetchKey::Commits(_) => data.commits.start_loading(),
            FetchKey::Diff(_) => data.diff.start_loading(),
            FetchKey::Builds(_) => data.builds.start_loading(),
            FetchKey::Activity(_) => data.activity.start_loading(),
            FetchKey::Mergeability(_) => data.mergeability.start_loading(),
            FetchKey::Info(_) => data.info.start_loading(),
            FetchKey::CommitDiff(_, oid) => data
                .commit_diffs
                .entry(oid.clone())
                .or_insert(LoadState::NotRequested)
                .start_loading(),
        }
    }

    /// Record a write for the PR. `None` while another is pending for it: one
    /// write per PR at a time, so its payload and review queue stay as sent.
    pub fn begin_write(&mut self, pr_id: u64, operation: Operation) -> Option<WriteTicket> {
        match self.operations.entry(pr_id) {
            std::collections::hash_map::Entry::Occupied(_) => None,
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(operation);
                Some(WriteTicket { pr_id, operation })
            }
        }
    }

    /// How many open PRs are read now: the first batch and what `L` added.
    pub const fn open_limit(&self) -> usize {
        OPEN_BATCH * (1 + self.open_extra)
    }

    /// Whether the group has been read. The open group counts as read as soon
    /// as the list itself has loaded.
    pub fn group_loaded(&self, group: PrGroup) -> bool {
        self.groups.get(&group).is_some_and(|state| state.loaded)
            || (group == PrGroup::Open && matches!(self.cache.prs, LoadState::Loaded(_)))
    }

    /// Whether older PRs remain to be read in the group.
    pub fn group_has_more(&self, group: PrGroup) -> bool {
        self.groups
            .get(&group)
            .is_some_and(|state| state.more.is_some())
    }

    pub fn group_loading(&self, group: PrGroup) -> bool {
        self.fetches.contains(&FetchKey::Prs(group))
    }

    pub fn refreshing(&self, screen: crate::app::navigation::Screen) -> bool {
        use crate::app::navigation::Screen;
        self.fetches.iter().any(|key| match (screen, key) {
            (_, FetchKey::Prs(_)) => true,
            (
                Screen::Detail { pr_id, .. },
                FetchKey::Commits(id)
                | FetchKey::Diff(id)
                | FetchKey::Builds(id)
                | FetchKey::Activity(id)
                | FetchKey::Mergeability(id)
                | FetchKey::Info(id)
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
            FetchKey::Prs(group) => return self.group_loaded(*group),
            FetchKey::Commits(id)
            | FetchKey::Diff(id)
            | FetchKey::Builds(id)
            | FetchKey::Activity(id)
            | FetchKey::Mergeability(id)
            | FetchKey::Info(id)
            | FetchKey::CommitDiff(id, _) => id,
        };
        let Some(data) = self.cache.details.get(id) else {
            return false;
        };
        match key {
            FetchKey::Prs(_) => false,
            FetchKey::Commits(_) => matches!(data.commits, LoadState::Loaded(_)),
            FetchKey::Diff(_) => matches!(data.diff, LoadState::Loaded(_)),
            FetchKey::Builds(_) => matches!(data.builds, LoadState::Loaded(_)),
            FetchKey::Activity(_) => matches!(data.activity, LoadState::Loaded(_)),
            FetchKey::Mergeability(_) => matches!(data.mergeability, LoadState::Loaded(_)),
            FetchKey::Info(_) => matches!(data.info, LoadState::Loaded(_)),
            FetchKey::CommitDiff(_, oid) => {
                matches!(data.commit_diffs.get(oid), Some(LoadState::Loaded(_)))
            }
        }
    }
    pub fn refresh_failed(&self, screen: crate::app::navigation::Screen) -> bool {
        use crate::{app::navigation::Screen, tui::screens::pr_detail::tabs::DetailTab};
        self.refresh_failures.iter().any(|key| match (screen, key) {
            (_, FetchKey::Prs(_)) => true,
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
            (Screen::Detail { pr_id, tab }, FetchKey::Info(id)) => {
                pr_id == *id && matches!(tab, DetailTab::Overview | DetailTab::Description)
            }
            (
                Screen::Detail { pr_id, tab },
                FetchKey::Commits(id) | FetchKey::CommitDiff(id, _),
            ) => pr_id == *id && tab == DetailTab::Commits,
            _ => false,
        })
    }
}
