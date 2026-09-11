use crate::{
    app::{
        navigation::Screen,
        store::{PrData, Store},
    },
    tui::Ui,
};

#[derive(Debug, Default)]
pub struct AppState {
    pub store: Store,
    pub ui: Ui,
    pub screen: Screen,
}

impl AppState {
    pub fn detail_view(&self) -> crate::tui::screens::pr_detail::DetailView<'_> {
        crate::tui::screens::pr_detail::DetailView {
            detail: &self.ui.detail,
            store: &self.store,
            screen: self.screen,
            refreshing: self.ui.refreshing,
        }
    }
    pub fn is_loading(&self) -> bool {
        self.ui.detail.comment_pending
            || self.store.cache.prs.is_loading()
            || self.store.cache.details.values().any(PrData::any_loading)
    }
}
