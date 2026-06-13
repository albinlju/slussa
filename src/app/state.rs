use std::collections::{HashMap, HashSet};

use crate::clients::ActivityBundle;
use crate::domain::ci::Build;
use crate::domain::commit::Commit;
use crate::domain::diff::Diff;
use crate::domain::pr::{PrStatus, PullRequest};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Description,
    Overview,
    Diff,
    Commits,
    Builds,
}

impl DetailTab {
    pub const ALL: [Self; 5] = [
        Self::Description,
        Self::Overview,
        Self::Diff,
        Self::Commits,
        Self::Builds,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let len = Self::ALL.len();
        Self::ALL[(self.index() + len - 1) % len]
    }
}

#[derive(Debug, Default)]
pub struct AppState {
    pub cache: Cache,
    pub ui: UiMemory,
    pub screen: Screen,
}

#[derive(Debug, Default)]
pub struct SearchState {
    pub open: bool,
    pub query: String,
}

impl SearchState {
    pub fn matches(&self, haystack: &str) -> bool {
        self.query.is_empty() || haystack.to_lowercase().contains(&self.query.to_lowercase())
    }

    pub fn matches_pr(&self, pr: &PullRequest) -> bool {
        self.matches(&pr.title)
            || self.matches(&pr.author.username)
            || self.matches(&format!("#{}", pr.id))
    }

    fn matches_commit(&self, c: &Commit) -> bool {
        self.matches(&c.oid) || self.matches(&c.headline)
    }

    pub fn filter_commits<'a>(&self, commits: &'a [Commit]) -> Vec<&'a Commit> {
        commits
            .iter()
            .filter(|c| self.matches_commit(c))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTarget {
    List,
    Commits,
    DiffTree,
    DiffPane,
}

#[derive(Debug, Default)]
pub struct UiMemory {
    pub list_selected: usize,
    pub list_viewport: u16,
    pub list_filter: StatusFilter,
    pub list_search: SearchState,
    pub filter_picker_open: bool,
    pub filter_picker_cursor: usize,
    pub diff: DiffViewState,
    pub commits: CommitsViewState,
    pub description_scroll: u16,
    pub overview_scroll: u16,
    pub description_viewport: u16,
    pub overview_viewport: u16,
}

impl UiMemory {
    pub fn active_diff_view(&self) -> &DiffViewState {
        if self.commits.drilled.is_some() {
            &self.commits.diff
        } else {
            &self.diff
        }
    }

    pub fn active_diff_view_mut(&mut self) -> &mut DiffViewState {
        if self.commits.drilled.is_some() {
            &mut self.commits.diff
        } else {
            &mut self.diff
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusFilter {
    #[default]
    Open,
    Draft,
    Merged,
    Declined,
    All,
}

impl StatusFilter {
    pub const CYCLE: [Self; 5] = [
        Self::Open,
        Self::Draft,
        Self::Merged,
        Self::Declined,
        Self::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Declined => "Declined",
            Self::All => "All",
        }
    }

    pub fn matches(self, status: &PrStatus) -> bool {
        matches!(
            (self, status),
            (Self::All, _)
                | (Self::Open, PrStatus::Open)
                | (Self::Draft, PrStatus::Draft)
                | (Self::Merged, PrStatus::Merged)
                | (Self::Declined, PrStatus::Declined)
        )
    }
}

#[derive(Debug, Default)]
pub struct DiffViewState {
    pub cursor: usize,
    pub focused_file: usize,
    pub collapsed: HashSet<String>,
    pub pane_scroll: u16,
    pub pane_cursor: usize,
    pub pane_viewport: u16,
    pub tree_viewport: u16,
    pub pane_items: usize,
    pub tree_search: SearchState,
    pub pane_search: SearchState,
    pub pane_matches: Vec<usize>,
    pub focus: DiffFocus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DiffFocus {
    #[default]
    Tree,
    Pane,
}

#[derive(Debug, Default)]
pub struct CommitsViewState {
    pub selected: usize,
    pub viewport: u16,
    pub search: SearchState,
    pub drilled: Option<String>,
    pub diff: DiffViewState,
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
    pub activity: LoadState<ActivityBundle>,
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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    List,
    Detail {
        pr_id: u64,
        tab: DetailTab,
    },
}

impl AppState {
    pub fn search_target(&self) -> Option<SearchTarget> {
        match self.screen {
            Screen::List => (!self.ui.filter_picker_open).then_some(SearchTarget::List),
            Screen::Detail { tab, .. } => {
                let drilled = self.ui.commits.drilled.is_some();
                match tab {
                    DetailTab::Commits if !drilled => Some(SearchTarget::Commits),
                    DetailTab::Diff | DetailTab::Commits => {
                        match self.ui.active_diff_view().focus {
                            DiffFocus::Tree => Some(SearchTarget::DiffTree),
                            DiffFocus::Pane => Some(SearchTarget::DiffPane),
                        }
                    }
                    _ => None,
                }
            }
        }
    }

    pub fn filtered_prs(&self) -> Vec<&PullRequest> {
        match &self.cache.prs {
            LoadState::Loaded(prs) => prs
                .iter()
                .filter(|pr| self.ui.list_filter.matches(&pr.status))
                .filter(|pr| self.ui.list_search.matches_pr(pr))
                .collect(),
            _ => Vec::new(),
        }
    }
}
