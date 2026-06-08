use std::collections::{HashMap, HashSet};

use crate::domain::ci::Build;
use crate::domain::comment::{Comment, ReviewThread};
use crate::domain::commit::Commit;
use crate::domain::diff::Diff;
use crate::domain::event::TimelineEvent;
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
    pub list_filter: StatusFilter,
    pub filter_picker_open: bool,
    pub filter_picker_cursor: usize,
    pub diff: DiffViewState,
    pub description_scroll: u16,
    pub overview_scroll: u16,
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
    /// Which side of the Diff tab has the keyboard — the file tree or the
    /// diff pane. Enter on a file hands focus to the pane; Esc/h hands it back.
    pub focus: DiffFocus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DiffFocus {
    #[default]
    Tree,
    Pane,
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
    pub comments: LoadState<Vec<Comment>>,
    pub review_threads: LoadState<Vec<ReviewThread>>,
    pub builds: LoadState<Vec<Build>>,
    pub events: LoadState<Vec<TimelineEvent>>,
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
