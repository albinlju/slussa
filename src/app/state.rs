use std::collections::{HashMap, HashSet};

use crate::clients::ActivityBundle;
use crate::domain::ci::Build;
use crate::domain::commit::Commit;
use crate::domain::diff::Diff;
use crate::domain::pr::{PrStatus, PullRequest};
use crate::tui::screens::pr_detail::DetailTab;

#[derive(Debug, Default)]
pub struct AppState {
    pub cache: Cache,
    pub ui: UiMemory,
    pub screen: Screen,
}

#[derive(Debug, Default)]
pub struct UiMemory {
    pub list_selected: usize,
    /// Last-rendered PR-list height, for half-page selection jumps.
    pub list_viewport: u16,
    pub list_filter: StatusFilter,
    pub filter_picker_open: bool,
    pub filter_picker_cursor: usize,
    pub diff: DiffViewState,
    pub commits: CommitsViewState,
    pub description_scroll: u16,
    pub overview_scroll: u16,
    /// Last-rendered content height of the description / overview views, so
    /// Ctrl+D/U can scroll by a half page.
    pub description_viewport: u16,
    pub overview_viewport: u16,
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
    /// Cursor within the focused file's diff, as a logical diff-line index
    /// (added/removed/context, in file order). The line a comment would anchor
    /// to; the pane highlights it and scrolls to keep it visible.
    pub pane_cursor: usize,
    /// Last-rendered body heights of the diff pane / file tree, for half-page
    /// scrolling and cursor jumps.
    pub pane_viewport: u16,
    pub tree_viewport: u16,
    /// Number of navigable items (diff lines + inline thread boxes) in the
    /// focused file, written by the pane each render. The reducer clamps
    /// `pane_cursor` against it so the cursor can step onto comment threads.
    pub pane_items: usize,
    pub focus: DiffFocus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DiffFocus {
    #[default]
    Tree,
    Pane,
}

/// State for the Commits tab. Its resting state is the commit list; pressing
/// Enter on a commit "drills in" to that commit's diff, rendered with the same
/// widget as the Diff tab via [`DiffViewState`]. The drill-in is transient —
/// leaving the tab clears `drilled` back to the list (see the reducer).
#[derive(Debug, Default)]
pub struct CommitsViewState {
    /// Cursor in the commit list.
    pub selected: usize,
    /// Last-rendered list height, for half-page selection jumps.
    pub viewport: u16,
    /// `Some(oid)` while viewing a single commit's diff; `None` = list view.
    pub drilled: Option<String>,
    /// Diff-pane state used while drilled in, kept separate from the Diff
    /// tab's own `diff` so the two views don't clobber each other's scroll.
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
    /// Comments, lifecycle events, and inline review threads — one fetch.
    pub activity: LoadState<ActivityBundle>,
    /// Per-commit diffs, fetched lazily when a commit is drilled into from the
    /// Commits tab. Keyed by commit oid.
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
        if matches!(self, LoadState::NotRequested) {
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
    pub fn filtered_prs(&self) -> Vec<&PullRequest> {
        match &self.cache.prs {
            LoadState::Loaded(prs) => prs
                .iter()
                .filter(|pr| self.ui.list_filter.matches(&pr.status))
                .collect(),
            _ => Vec::new(),
        }
    }
}
