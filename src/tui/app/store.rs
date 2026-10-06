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
        authorship::AiMarkers,
        build_log::BuildLog,
        capabilities::{Capabilities, Feature},
        ci::{Build, JobId},
        commit::{Commit, CommitOid},
        diff::Diff,
        pr::{Mergeability, PrGroup, PrId, PrInfo, PrStatus, PullRequest},
        seen::Seen,
        user::Username,
    },
    local::proposals::Proposals,
    providers::FetchError,
};
use std::collections::{HashMap, HashSet};

pub use super::notice::{Notice, NoticeKind};
use super::pr_groups::{GroupState, OpenChain};

#[derive(Debug)]
pub struct Store {
    pub refresh_failures: HashSet<FetchKey>,
    pub link_pending: bool,
    pub notice: Option<Notice>,
    pub draft_error: Option<String>,
    pub uncertain_submissions: std::collections::BTreeSet<PrId>,
    pub operations: HashMap<PrId, Operation>,
    pub errors: HashMap<PrId, String>,
    pub fetches: HashSet<FetchKey>,
    pub reload_after_fetch: HashSet<FetchKey>,
    pub reviews: HashMap<PrId, crate::domain::review::PendingReview>,
    /// When each PR was last looked at, to mark the ones changed since.
    pub seen: Seen,
    /// What agents have proposed, as the file said when it was last read. Which of
    /// it the reader has dealt with is in `seen`.
    pub proposals: Proposals,
    /// The command that reviews a PR when asked to; empty where there is none.
    pub agent_review: Vec<String>,
    /// A file with the instructions the reader wants an asked-for review to follow,
    /// instead of the built-in ones.
    pub agent_review_instructions: Option<std::path::PathBuf>,
    /// The PR the reader named on the command line, read and waiting for the
    /// list to be read before it is opened.
    pub requested: Option<PullRequest>,
    pub cache: Cache,
    /// What has been read of each group of PRs.
    pub groups: HashMap<PrGroup, GroupState>,
    pub open_chain: OpenChain,
    /// How many more batches of open PRs `L` has asked for.
    pub open_extra: usize,
    pub current_user: Username,
    pub capabilities: Capabilities,
    /// How comments by an AI agent are recognised; empty unless configured.
    /// Private: `set_ai_markers` judges what is cached, and `judged` what arrives.
    ai_markers: AiMarkers,
}

impl Store {
    /// Use these markers from now on, and judge the activity and the commits
    /// already read with them.
    pub fn set_ai_markers(&mut self, markers: AiMarkers) {
        self.ai_markers = markers;
        for data in self.cache.details.values_mut() {
            if let LoadState::Loaded(activity) = &mut data.activity {
                self.ai_markers.judge(activity);
            }
            if let LoadState::Loaded(commits) = &mut data.commits {
                self.ai_markers.judge_commits(commits);
            }
        }
    }

    /// Commits that have arrived, each judged once, here.
    pub fn judged_commits(&self, mut commits: Vec<Commit>) -> Vec<Commit> {
        self.ai_markers.judge_commits(&mut commits);
        commits
    }

    /// An activity that has arrived, with each comment judged once, here.
    pub fn judged(&self, mut activity: Activity) -> Activity {
        self.ai_markers.judge(&mut activity);
        activity
    }

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
            seen: Seen::new(),
            proposals: Proposals::default(),
            agent_review: Vec::new(),
            agent_review_instructions: None,
            requested: None,
            cache: Cache::default(),
            groups: HashMap::new(),
            open_chain: OpenChain::default(),
            open_extra: 0,
            current_user,
            capabilities,
            ai_markers: AiMarkers::default(),
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

#[derive(Debug, Default)]
pub struct Cache {
    pub prs: LoadState<Vec<PullRequest>>,
    pub details: HashMap<PrId, PrData>,
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
    pub commit_diffs: HashMap<CommitOid, LoadState<Diff>>,
    /// The logs of the builds the reader opened.
    pub build_logs: HashMap<JobId, LoadState<BuildLog>>,
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
            || self.build_logs.values().any(LoadState::is_loading)
    }
}

impl PrData {
    /// Mark the resource as loading if it has never been read (or failed), and
    /// say whether to read it now.
    fn start_loading(&mut self, resource: &PrResource) -> bool {
        match resource {
            PrResource::Commits => self.commits.start_loading(),
            PrResource::Diff => self.diff.start_loading(),
            PrResource::Builds => self.builds.start_loading(),
            PrResource::Activity => self.activity.start_loading(),
            PrResource::Mergeability => self.mergeability.start_loading(),
            PrResource::Info => self.info.start_loading(),
            PrResource::CommitDiff(oid) => self
                .commit_diffs
                .entry(oid.clone())
                .or_insert(LoadState::NotRequested)
                .start_loading(),
            PrResource::BuildLog(job) => self
                .build_logs
                .entry(*job)
                .or_insert(LoadState::NotRequested)
                .start_loading(),
            // Asked for by the reader, never read because a PR was opened.
            PrResource::AgentReview => false,
        }
    }

    fn has_loaded(&self, resource: &PrResource) -> bool {
        match resource {
            PrResource::Commits => self.commits.loaded().is_some(),
            PrResource::Diff => self.diff.loaded().is_some(),
            PrResource::Builds => self.builds.loaded().is_some(),
            PrResource::Activity => self.activity.loaded().is_some(),
            PrResource::Mergeability => self.mergeability.loaded().is_some(),
            PrResource::Info => self.info.loaded().is_some(),
            PrResource::CommitDiff(oid) => self
                .commit_diffs
                .get(oid)
                .is_some_and(|state| state.loaded().is_some()),
            PrResource::BuildLog(job) => self
                .build_logs
                .get(job)
                .is_some_and(|state| state.loaded().is_some()),
            PrResource::AgentReview => false,
        }
    }

    pub fn diff_for(&self, commit: Option<&CommitOid>) -> Option<&LoadState<Diff>> {
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
    AutoMerge,
    CancelAutoMerge,
    RerunBuilds,
    RerequestReview,
    Decline,
    Reopen,
}

impl Operation {
    /// What the notice says once it went through.
    pub const fn done_label(self) -> &'static str {
        match self {
            Self::Merge => "merged",
            Self::AutoMerge => "will merge when ready",
            Self::CancelAutoMerge => "auto-merge off",
            Self::RerunBuilds => "failed builds run again",
            Self::RerequestReview => "asked to review again",
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
            // Whether it is a draft or has a conflict is read with the list again.
            Self::Reopen => Some(PrStatus::open()),
            Self::Comment
            | Self::Moderation
            | Self::Review
            | Self::AutoMerge
            | Self::CancelAutoMerge
            | Self::RerunBuilds
            | Self::RerequestReview => None,
        }
    }

    /// Whether it sends what is in the comment editor.
    pub const fn sends_editor_text(self) -> bool {
        match self {
            Self::Comment | Self::Review => true,
            Self::Moderation
            | Self::Merge
            | Self::AutoMerge
            | Self::CancelAutoMerge
            | Self::RerunBuilds
            | Self::RerequestReview
            | Self::Decline
            | Self::Reopen => false,
        }
    }
}

/// Proof that a read is registered in `Store::fetches`. Only
/// `Store::begin_fetch` makes one, and starting the read takes it, so a read
/// cannot be spawned without being registered first.
#[derive(Debug)]
#[must_use = "a registered read that is never started stays loading"]
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
#[must_use = "a recorded write that is never started blocks the PR"]
pub struct WriteTicket {
    pr_id: PrId,
    operation: Operation,
}

impl WriteTicket {
    pub const fn pr_id(&self) -> PrId {
        self.pr_id
    }

    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// The ticket of a write already recorded, for a test that plays its worker.
    #[cfg(test)]
    pub const fn pending(pr_id: PrId, operation: Operation) -> Self {
        Self { pr_id, operation }
    }
}

/// One of the things read about a single PR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrResource {
    Commits,
    Diff,
    Builds,
    Activity,
    Mergeability,
    Info,
    CommitDiff(CommitOid),
    /// An agent asked to review the PR. Nothing is stored in the PR's data: what
    /// it proposes is kept in a file, which is read again when it is done.
    AgentReview,
    /// Read when a build is opened and not again: a log does not change.
    BuildLog(JobId),
}

impl PrResource {
    /// The provider feature this resource needs, if it is an optional one.
    const fn feature(&self) -> Option<Feature> {
        match self {
            Self::Builds => Some(Feature::Builds),
            Self::Mergeability => Some(Feature::Mergeability),
            Self::AgentReview => Some(Feature::AgentReview),
            Self::Info => Some(Feature::PrInfo),
            Self::Commits
            | Self::Diff
            | Self::Activity
            | Self::CommitDiff(_)
            | Self::BuildLog(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchKey {
    /// One group of PRs, read first or, for a closed group, one page further.
    Prs(PrGroup),
    /// One PR by its number, asked for by the reader.
    One(PrId),
    Pr(PrResource, PrId),
}

impl Store {
    /// When the PR was last updated, as the list read it.
    pub fn pr_updated(&self, pr_id: PrId) -> Option<chrono::DateTime<chrono::Utc>> {
        self.cache
            .prs
            .loaded()?
            .iter()
            .find(|pr| pr.id == pr_id)
            .map(|pr| pr.updated)
    }

    /// Whether the provider has this resource at all.
    fn offers(&self, key: &FetchKey) -> bool {
        match key {
            FetchKey::Prs(_) | FetchKey::One(_) => true,
            FetchKey::Pr(resource, _) => resource
                .feature()
                .is_none_or(|feature| self.capabilities.supports(feature)),
        }
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
        match key {
            FetchKey::Prs(_) => self.cache.prs.start_loading(),
            // Asked for once, by number; there is no state to mark.
            FetchKey::One(_) => false,
            FetchKey::Pr(resource, id) => self
                .cache
                .details
                .entry(*id)
                .or_default()
                .start_loading(resource),
        }
    }

    /// Record a write for the PR. `None` while another is pending for it: one
    /// write per PR at a time, so its payload and review queue stay as sent.
    pub fn begin_write(&mut self, pr_id: PrId, operation: Operation) -> Option<WriteTicket> {
        match self.operations.entry(pr_id) {
            std::collections::hash_map::Entry::Occupied(_) => None,
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(operation);
                Some(WriteTicket { pr_id, operation })
            }
        }
    }

    pub fn refreshing(&self, screen: crate::tui::app::navigation::Screen) -> bool {
        use crate::tui::app::navigation::Screen;
        self.fetches.iter().any(|key| match key {
            FetchKey::Prs(_) => true,
            FetchKey::One(_) => false,
            FetchKey::Pr(_, id) => {
                matches!(screen, Screen::Detail { pr_id, .. } if pr_id == *id)
            }
        })
    }
}

impl Store {
    pub fn has_cached_data(&self, key: &FetchKey) -> bool {
        match key {
            FetchKey::Prs(group) => self.group_loaded(*group),
            FetchKey::One(id) => self
                .cache
                .prs
                .loaded()
                .is_some_and(|prs| prs.iter().any(|pr| pr.id == *id)),
            FetchKey::Pr(resource, id) => self
                .cache
                .details
                .get(id)
                .is_some_and(|data| data.has_loaded(resource)),
        }
    }
    pub fn refresh_failed(&self, screen: crate::tui::app::navigation::Screen) -> bool {
        use crate::tui::{app::navigation::Screen, ui::screens::pr_detail::tabs::DetailTab};
        // Whether the screen is this PR on one of the tabs that show the data.
        let on = |id: &PrId, shows: fn(DetailTab) -> bool| matches!(screen, Screen::Detail { pr_id, tab } if pr_id == *id && shows(tab));
        self.refresh_failures.iter().any(|key| match key {
            FetchKey::Prs(_) => true,
            // A review's failure is its own error on the PR, not a stale view.
            FetchKey::One(_) | FetchKey::Pr(PrResource::AgentReview, _) => false,
            FetchKey::Pr(PrResource::Diff, id) => on(id, |tab| tab == DetailTab::Diff),
            FetchKey::Pr(PrResource::Activity, id) => on(id, |tab| {
                matches!(
                    tab,
                    DetailTab::Overview | DetailTab::Diff | DetailTab::Commits
                )
            }),
            FetchKey::Pr(PrResource::Builds, id) => on(id, |tab| {
                matches!(tab, DetailTab::Overview | DetailTab::Builds)
            }),
            FetchKey::Pr(PrResource::BuildLog(_), id) => on(id, |tab| tab == DetailTab::Builds),
            FetchKey::Pr(PrResource::Mergeability, id) => on(id, |_| true),
            FetchKey::Pr(PrResource::Info, id) => on(id, |tab| {
                matches!(tab, DetailTab::Overview | DetailTab::Description)
            }),
            FetchKey::Pr(PrResource::Commits | PrResource::CommitDiff(_), id) => {
                on(id, |tab| tab == DetailTab::Commits)
            }
        })
    }
}
