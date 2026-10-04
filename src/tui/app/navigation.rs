use crate::{
    domain::pr::PrId,
    tui::{
        app::{
            App,
            store::{FetchKey, PrResource},
        },
        ui::screens::pr_detail::tabs::DetailTab,
    },
};

impl App {
    pub(super) fn open_pr(&mut self, pr_id: PrId) {
        self.state.ui.open_pr(pr_id);
        self.state.screen = Screen::Detail {
            pr_id,
            tab: self.state.ui.detail.active_tab,
        };
        self.mark_viewed_seen();
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
