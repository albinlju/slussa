use std::collections::HashMap;

use crate::domain::commit::Commit;
use crate::domain::pr::PullRequest;
use crate::tui::pr_detail::DetailTab;

#[derive(Debug, Default)]
pub struct AppState {
    pub cache: Cache,
    pub selected: usize,
    pub screen: Screen,
}

#[derive(Debug, Default)]
pub struct Cache {
    pub prs: LoadState<Vec<PullRequest>>,
    pub details: HashMap<u64, PrData>,
}

#[derive(Debug, Default)]
pub struct PrData {
    pub commits: LoadState<Vec<Commit>>,
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
