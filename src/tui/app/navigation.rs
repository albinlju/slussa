use crate::{
    domain::pr::{PrGroup, PrId, PullRequest},
    providers::FetchError,
    tui::{
        app::{
            App,
            store::{FetchKey, LoadState, Notice, PrResource},
        },
        ui::screens::pr_detail::tabs::DetailTab,
    },
};

impl App {
    /// Start on the PR the reader named, if one: the PR screen at once, with its
    /// parts and the PR itself read together, rather than the list first.
    pub(super) fn start_opening(&mut self) {
        if let Some(pr_id) = self.start_on.take() {
            self.open_named_pr(pr_id);
        }
    }

    /// Open a PR that may not be in the list: the PR itself is read too, then.
    pub(super) fn open_named_pr(&mut self, pr_id: PrId) {
        let listed = self
            .state
            .store
            .cache
            .prs
            .loaded()
            .is_some_and(|prs| prs.iter().any(|pr| pr.id == pr_id));
        self.open_pr(pr_id);
        if !listed {
            self.spawn_load_pr(pr_id);
        }
    }

    /// The PR the reader named has been read, or could not be.
    pub(super) fn requested_pr_read(
        &mut self,
        pr_id: PrId,
        result: Result<PullRequest, FetchError>,
    ) {
        match result {
            Ok(pr) => {
                self.state.store.requested = Some(pr);
                self.place_requested_pr();
            }
            Err(error) => {
                self.state.store.notice = Some(Notice::error(format!(
                    "Couldn't open PR #{pr_id}: {}",
                    error.user_message()
                )));
                // There is no PR to show, so back to the list.
                if matches!(self.state.screen, Screen::Detail { pr_id: shown, .. } if shown == pr_id)
                {
                    self.state.screen = Screen::List;
                }
            }
        }
    }

    /// Put the PR the reader named in the list, once the list is read: a PR on
    /// screen has to be in it. Until then the screen says it is loading. A reader
    /// who has gone back to the list meanwhile is not taken to the PR.
    pub(super) fn place_requested_pr(&mut self) {
        let store = &mut self.state.store;
        let LoadState::Loaded(prs) = &mut store.cache.prs else {
            return;
        };
        let Some(pr) = store.requested.take() else {
            return;
        };
        // The one just read is at least as new as one the list holds, and is the
        // one the screen may already be showing.
        if let Some(known) = prs.iter_mut().find(|known| known.id == pr.id) {
            *known = pr;
        } else {
            prs.push(pr);
            prs.sort_by_key(|pr| PrGroup::of(&pr.status));
        }
        self.mark_viewed_seen();
        self.mark_read_head();
    }

    pub(super) fn open_pr(&mut self, pr_id: PrId) {
        self.state.ui.open_pr(pr_id);
        self.state.screen = Screen::Detail {
            pr_id,
            tab: self.state.ui.detail.active_tab,
        };
        self.mark_viewed_seen();
        self.mark_read_head();
        // Whatever this PR has not had read yet; the rest is shown from cache.
        for key in [
            FetchKey::Pr(PrResource::Commits, pr_id),
            FetchKey::Pr(PrResource::Diff, pr_id),
            FetchKey::Pr(PrResource::Builds, pr_id),
            FetchKey::Pr(PrResource::Activity, pr_id),
            FetchKey::Pr(PrResource::Info, pr_id),
            FetchKey::Pr(PrResource::Mergeability, pr_id),
        ] {
            self.ensure_loaded(key);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    List,
    Detail {
        pr_id: PrId,
        tab: DetailTab,
    },
}
