use crate::{
    app::{
        navigation::Screen,
        store::{PrData, Store},
    },
    tui::Ui,
};

#[derive(Debug)]
#[cfg_attr(test, derive(Default))]
pub struct AppState {
    pub store: Store,
    pub ui: Ui,
    pub screen: Screen,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            ui: Ui::default(),
            screen: Screen::default(),
        }
    }

    /// The PR screen's read-only view, for a test on that screen.
    #[cfg(test)]
    pub fn detail_view(&self) -> crate::tui::screens::pr_detail::DetailView<'_> {
        use crate::tui::screens::pr_detail::{DetailContext, DetailView};
        let Screen::Detail { pr_id, tab } = self.screen else {
            panic!("the test is not on the PR screen");
        };
        let ctx =
            DetailContext::new(&self.store, pr_id, tab).expect("the PR on screen is in the list");
        DetailView::new(&self.ui.detail, &ctx)
    }
    pub fn is_loading(&self) -> bool {
        !self.store.operations.is_empty()
            || !self.store.fetches.is_empty()
            || self.store.cache.prs.is_loading()
            || self.store.cache.details.values().any(PrData::any_loading)
    }
}
