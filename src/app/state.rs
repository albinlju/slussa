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

/// The `*_viewport` fields are written by the renderer each frame (last-drawn
/// content height) and read by the key handlers to size half-page jumps.
#[derive(Debug, Default)]
pub struct UiMemory {
    pub list_selected: usize,
    pub list_viewport: u16,
    pub list_filter: StatusFilter,
    pub filter_picker_open: bool,
    pub filter_picker_cursor: usize,
    pub diff: DiffViewState,
    pub commits: CommitsViewState,
    pub description_scroll: u16,
    pub overview_scroll: u16,
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
    /// Cursor over the focused file's navigable items (diff lines + inline
    /// thread boxes), in render order.
    pub pane_cursor: usize,
    pub pane_viewport: u16,
    pub tree_viewport: u16,
    /// Item count behind `pane_cursor`, written by the pane each render so
    /// the reducer can clamp the cursor.
    pub pane_items: usize,
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
    /// `Some(oid)` while viewing a single commit's diff; `None` = list view.
    pub drilled: Option<String>,
    /// Pane state for the drill-in, separate from the Diff tab's so the two
    /// views don't clobber each other's scroll.
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
    /// Per-commit diffs, fetched lazily on drill-in. Keyed by commit oid.
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
    /// Flip to `Loading` and return true when a fetch should start: not yet
    /// requested, or failed (so reopening retries).
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
