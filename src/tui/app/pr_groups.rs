//! Reading the PR list: the groups, the pages of a group, and what `L` asks
//! for. The open group is read page by page up to a limit; a closed group is
//! read one page at a time, newest first, and goes further when asked.
//! `spawn_load_prs` in `fetchers` starts each read.

use crate::{
    domain::pr::{PrBatch, PrGroup, PullRequest},
    providers::FetchError,
    tui::{
        app::{
            App,
            navigation::Screen,
            store::{FetchKey, LoadState, Notice, Store},
        },
        ui::screens::pr_list::ListContext,
    },
};

/// How many open PRs are read without being asked to, and how many each `L`
/// adds. Three pages on GitHub. A repository with fewer is read in full.
pub const OPEN_BATCH: usize = 90;

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

impl Store {
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
}

impl App {
    /// Read whatever the current view shows that has not been read yet.
    pub(super) fn ensure_view_loaded(&mut self) {
        for &group in self.state.ui.list.filter.groups() {
            if !self.state.store.group_loaded(group) {
                self.spawn_load_prs(group, None);
            }
        }
    }

    /// `L`: the next batch of each group in the view that has more.
    pub(super) fn load_older_prs(&mut self) {
        for &group in self.state.ui.list.filter.groups() {
            if group == PrGroup::Open {
                self.load_more_open();
                continue;
            }
            let more = self
                .state
                .store
                .groups
                .get(&group)
                .and_then(|state| state.more.clone());
            if let Some(after) = more {
                self.spawn_load_prs(group, Some(after));
            }
        }
    }

    /// Read on in the open group, the next batch past what is shown.
    fn load_more_open(&mut self) {
        let store = &mut self.state.store;
        let Some(after) = store
            .groups
            .get(&PrGroup::Open)
            .and_then(|state| state.more.clone())
        else {
            return;
        };
        if store.group_loading(PrGroup::Open) || !matches!(store.open_chain, OpenChain::Idle) {
            return;
        }
        store.open_extra += 1;
        store.open_chain = OpenChain::Appending;
        self.spawn_load_prs(PrGroup::Open, Some(after));
    }

    /// Read the open group again, and every other group already read.
    pub(super) fn refresh_list(&mut self) {
        for group in PrGroup::ALL {
            if group == PrGroup::Open || self.state.store.group_loaded(group) {
                self.spawn_load_prs(group, None);
            }
        }
    }

    pub(super) fn prs_loaded(
        &mut self,
        group: PrGroup,
        continuation: bool,
        result: Result<PrBatch, FetchError>,
    ) {
        let older = continuation && group.is_paged();
        // The highlighted PR keeps the highlight wherever its row ends up.
        let selected_id = self
            .list_rows()
            .get(self.state.ui.list.selected)
            .map(|pr| pr.id);
        if group == PrGroup::Open && result.is_err() {
            self.state.store.open_chain = OpenChain::Idle;
        }
        match result {
            Ok(batch) if group == PrGroup::Open => self.adopt_open_page(continuation, batch),
            Ok(batch) => self.adopt_group(group, older, batch),
            Err(error) if older => {
                self.state.store.notice = Some(Notice::error(format!(
                    "Couldn't load older PRs: {}",
                    error.user_message()
                )));
            }
            // The open group is the list itself: a first failure shows as a
            // failed list, a failed refresh keeps what is there.
            Err(error) if group == PrGroup::Open => {
                self.state.store.cache.prs.reload(Err(error));
            }
            // A closed group never read before has no data to keep, so say so.
            Err(error) if !self.state.store.group_loaded(group) => {
                self.state.store.notice = Some(Notice::error(format!(
                    "Couldn't load {} PRs: {}",
                    group.label(),
                    error.user_message()
                )));
            }
            // A failed refresh of a group already read is recorded in
            // `refresh_failures` above.
            Err(_) => {}
        }
        let filtered = self.list_rows();
        let selected = selected_id
            .and_then(|id| filtered.iter().position(|pr| pr.id == id))
            .unwrap_or_else(|| {
                self.state
                    .ui
                    .list
                    .selected
                    .min(filtered.len().saturating_sub(1))
            });
        self.state.ui.list.selected = selected;
        // A PR on screen while the list moves on has been seen at its newest.
        self.mark_viewed_seen();
    }

    /// The list's rows as it shows them now.
    fn list_rows(&self) -> Vec<&PullRequest> {
        let state = &self.state;
        let ctx = ListContext::from_store(&state.store, state.ui.list.filter, state.screen);
        state.ui.list.filtered_prs(&ctx)
    }

    /// One page of the open group. A first reading shows each page as it
    /// arrives, in arrival order. A refresh holds the pages and swaps them in
    /// after the last one. Either way the next page is asked for until there is
    /// none or the limit is reached, and then the list takes its order.
    fn adopt_open_page(&mut self, continuation: bool, batch: PrBatch) {
        let more = batch.more.clone();
        let store = &mut self.state.store;
        let chain = match (continuation, std::mem::take(&mut store.open_chain)) {
            (false, _) if matches!(store.cache.prs, LoadState::Loaded(_)) => {
                OpenChain::Collecting(batch.prs)
            }
            (false, _) => {
                self.adopt_group(PrGroup::Open, false, batch);
                OpenChain::Appending
            }
            (true, OpenChain::Appending) => {
                if let LoadState::Loaded(prs) = &mut store.cache.prs {
                    for pr in batch.prs {
                        if !prs.iter().any(|known| known.id == pr.id) {
                            prs.push(pr);
                        }
                    }
                    prs.sort_by_key(|pr| PrGroup::of(&pr.status));
                }
                OpenChain::Appending
            }
            (true, OpenChain::Collecting(mut held)) => {
                held.extend(batch.prs);
                OpenChain::Collecting(held)
            }
            // A page that belongs to a reading that has already ended.
            (true, OpenChain::Idle) => return,
        };
        let store = &mut self.state.store;
        let read = match &chain {
            OpenChain::Collecting(held) => held.len(),
            OpenChain::Idle | OpenChain::Appending => store.cache.prs.loaded().map_or(0, |prs| {
                prs.iter()
                    .filter(|pr| PrGroup::of(&pr.status) == PrGroup::Open)
                    .count()
            }),
        };
        let next = more.clone().filter(|_| read < store.open_limit());
        // Where `L` continues from, if the reading stops with more left.
        store
            .groups
            .entry(PrGroup::Open)
            .or_default()
            .more
            .clone_from(&more);
        if let Some(after) = next {
            self.state.store.open_chain = chain;
            self.spawn_load_prs(PrGroup::Open, Some(after));
            return;
        }
        if let OpenChain::Collecting(held) = chain {
            self.adopt_group(PrGroup::Open, false, PrBatch { prs: held, more });
        }
    }

    /// Take a group's read into the list. A first read replaces the group's
    /// PRs; older closed PRs already loaded stay, and so does the position
    /// reached, or a refresh every minute would throw them away. An older page
    /// goes after what is there. The PR open in the detail screen is never
    /// dropped, so a merge by someone else does not blank the screen.
    fn adopt_group(&mut self, group: PrGroup, older: bool, batch: PrBatch) {
        let viewing = match self.state.screen {
            Screen::Detail { pr_id, .. } => Some(pr_id),
            Screen::List => None,
        };
        let store = &mut self.state.store;
        let existing = match &mut store.cache.prs {
            LoadState::Loaded(prs) => std::mem::take(prs),
            LoadState::NotRequested | LoadState::Loading | LoadState::Failed(_) => Vec::new(),
        };
        let state = store.groups.entry(group).or_default();
        let mut notice = None;
        let mut prs = if older {
            let mut prs = existing;
            let mut added = 0;
            for pr in batch.prs {
                if !prs.iter().any(|known| known.id == pr.id) {
                    prs.push(pr);
                    added += 1;
                }
            }
            state.more = batch.more;
            state.older_loaded = true;
            notice = Some(match added {
                0 => "No older PRs".to_owned(),
                1 => "Loaded 1 older PR".to_owned(),
                n => format!("Loaded {n} older PRs"),
            });
            prs
        } else {
            let keep_older = state.older_loaded && group.is_paged();
            let mut prs = batch.prs;
            for old in existing {
                if prs.iter().any(|fresh| fresh.id == old.id) {
                    continue;
                }
                let in_group = PrGroup::of(&old.status) == group;
                if !in_group || keep_older || Some(old.id) == viewing {
                    prs.push(old);
                }
            }
            if !state.older_loaded {
                state.more = batch.more;
            }
            prs
        };
        state.loaded = true;
        // Providers give each group newest first; keep the groups in a fixed order.
        prs.sort_by_key(|pr| PrGroup::of(&pr.status));
        store.cache.prs = LoadState::Loaded(prs);
        if let Some(message) = notice {
            store.notice = Some(Notice::info(message));
        }
    }
}
