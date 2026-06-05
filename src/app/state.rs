use std::collections::{HashMap, HashSet};

use crate::domain::commit::Commit;
use crate::domain::diff::Diff;
use crate::domain::pr::PullRequest;
use crate::tui::pr_detail::DetailTab;

#[derive(Debug, Default)]
pub struct AppState {
    pub cache: Cache,
    pub ui: UiMemory,
    pub screen: Screen,
}

#[derive(Debug, Default)]
pub struct UiMemory {
    pub list_selected: usize,
    pub diff: DiffViewState,
    pub description_expanded: bool,
    pub description_scroll: u16,
}

#[derive(Debug, Default)]
pub struct DiffViewState {
    pub cursor: usize,
    pub focused_file: usize,
    pub collapsed: HashSet<String>,
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
}

#[derive(Debug, Default)]
pub enum LoadState<T> {
    #[default]
    NotRequested,
    Loading,
    Loaded(T),
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
