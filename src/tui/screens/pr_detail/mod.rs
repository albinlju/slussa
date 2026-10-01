mod build_status;
pub mod dialogs;
mod footer;
mod header;
mod interactions;
pub mod keys;
mod render;
pub mod tabs;
pub mod view;
use crate::{
    app::{
        action::{
            Action, CommitsAction, DetailAction, DiffAction, EditorAction, Effect, ErrorAction,
            NavAction, SearchAction,
        },
        navigation::Screen,
    },
    tui::{
        component::Component,
        components::{
            diff_viewer::DiffViewer,
            search_input::{SearchInput, SearchKind},
        },
        screens::pr_detail::tabs::commits::CommitList,
    },
};
use ratatui::{Frame, layout::Rect};

pub use view::{DetailContext, DetailView};
#[derive(Debug, Default)]
pub struct PrDetailScreen {
    pub overview: tabs::overview::Overview,
    pub builds: tabs::builds::Builds,
    pub description: tabs::description::Description,
    pub confirm: Option<dialogs::confirm::ConfirmDialog>,
    pub review_picker: Option<dialogs::review::ReviewDialog>,
    pub merge_picker: Option<dialogs::merge::MergeDialog>,
    pub diff: DiffViewer,
    pub commits: CommitList,
    pub error: dialogs::error::ErrorDialog,
    pub help_open: bool,
    pub help: crate::tui::components::help_dialog::HelpDialog,
    pub editor: crate::tui::components::comment_editor::CommentEditor,
    pr_id: Option<u64>,
    pub active_tab: tabs::DetailTab,
    navigation: std::collections::HashMap<u64, DetailNavigation>,
    editors: std::collections::HashMap<u64, crate::tui::components::comment_editor::CommentEditor>,
}

#[derive(Debug, Default)]
struct DetailNavigation {
    overview: tabs::overview::Overview,
    builds: tabs::builds::Builds,
    description: tabs::description::Description,
    diff: DiffViewer,
    commits: CommitList,
    tab: tabs::DetailTab,
}

impl PrDetailScreen {
    pub fn reconcile_commits(
        &mut self,
        pr_id: u64,
        old: &[crate::domain::commit::Commit],
        new: &[crate::domain::commit::Commit],
    ) {
        if self.pr_id == Some(pr_id) {
            self.commits.reconcile(old, new);
        } else if let Some(position) = self.navigation.get_mut(&pr_id) {
            position.commits.reconcile(old, new);
        }
    }
    pub const fn modal_open(&self) -> bool {
        self.confirm.is_some()
            || self.review_picker.is_some()
            || self.merge_picker.is_some()
            || self.help_open
            || self.editor.is_open()
    }

    pub const fn active_diff_view(&self) -> &DiffViewer {
        if self.commits.open_commit.is_some() {
            &self.commits.diff
        } else {
            &self.diff
        }
    }

    pub const fn active_diff_view_mut(&mut self) -> &mut DiffViewer {
        if self.commits.open_commit.is_some() {
            &mut self.commits.diff
        } else {
            &mut self.diff
        }
    }
}

impl Component for PrDetailScreen {
    type Input<'a> = DetailContext<'a>;
    type View<'a> = DetailContext<'a>;
    type Message = DetailAction;
    fn handle_key(
        &self,
        key: crossterm::event::KeyEvent,
        ctx: &DetailContext<'_>,
    ) -> Option<Action> {
        let view = self.view(ctx);
        keys::key_to_action(&view, key).filter(|action| match action {
            Action::Detail(action) => view.supports_action(*action),
            Action::HelpScroll(_)
            | Action::Paste(_)
            | Action::List(_)
            | Action::Diff(_)
            | Action::Commits(_)
            | Action::Search(_)
            | Action::Effect(_) => true,
        })
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &DetailContext<'_>) {
        render::render(self, frame, area, ctx);
    }

    fn update(&mut self, action: DetailAction, ctx: &DetailContext<'_>) -> Option<Effect> {
        if !self.view(ctx).supports_action(action) {
            return None;
        }
        let (pr_id, tab) = (ctx.pr_id, ctx.tab);
        if ctx.store.operations.contains_key(&pr_id) && !action.allowed_while_sending() {
            return None;
        }
        match action {
            DetailAction::Nav(action) => return self.navigate(action, pr_id, tab, ctx),
            DetailAction::BuildsScroll(delta) => {
                self.builds.update(delta, &());
            }
            DetailAction::Description(action) => {
                self.description.update(action, &());
            }
            DetailAction::Timeline(action) => {
                self.overview.update(action, &());
            }
            DetailAction::Error(ErrorAction::Dismiss) => return Some(self.dismiss_error(pr_id)),
            DetailAction::Error(action @ ErrorAction::Scroll(_)) => {
                self.error.update(action, &());
            }
            DetailAction::Confirm(action) => return self.confirm_action(action, pr_id),
            DetailAction::Review(action) => return self.review_action(action, pr_id, ctx),
            DetailAction::Merge(action) => return self.merge_action(action, pr_id, ctx),
            DetailAction::Editor(EditorAction::Submit) => return self.submit_editor(pr_id),
            DetailAction::Editor(action) => {
                self.editor.update(action, &());
            }
            DetailAction::Pr(action) => return self.pr_action(action, pr_id, ctx),
        }
        None
    }
}

impl PrDetailScreen {
    fn navigate(
        &mut self,
        action: NavAction,
        pr_id: u64,
        tab: tabs::DetailTab,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let tab = match action {
            NavAction::Back => {
                self.help_open = false;
                return Some(Effect::Navigate(Screen::List));
            }
            NavAction::ToggleHelp => {
                self.help_open = !self.help_open;
                self.help = crate::tui::components::help_dialog::HelpDialog::default();
                return None;
            }
            NavAction::NextTab => tab.step(1, &ctx.store.capabilities),
            NavAction::PrevTab => tab.step(-1, &ctx.store.capabilities),
            NavAction::SelectTab(tab) => tab,
        };
        self.active_tab = tab;
        self.commits.open_commit = None;
        Some(Effect::Navigate(Screen::Detail { pr_id, tab }))
    }

    pub const fn active_search(&self, tab: tabs::DetailTab) -> Option<(&SearchInput, SearchKind)> {
        use tabs::DetailTab;
        match tab {
            DetailTab::Commits if self.commits.open_commit.is_none() => {
                Some((&self.commits.search, SearchKind::Filter))
            }
            DetailTab::Diff | DetailTab::Commits => Some(self.active_diff_view().active_search()),
            DetailTab::Description | DetailTab::Overview | DetailTab::Builds => None,
        }
    }

    pub fn update_commits(
        &mut self,
        action: CommitsAction,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        self.commits.update(
            action,
            &tabs::commits::CommitInput::new(ctx.pr_id, ctx.data),
        )
    }

    pub fn update_diff(&mut self, action: DiffAction, ctx: &DetailContext<'_>) -> Option<Effect> {
        let files = view::diff_files(ctx.data, self.commits.open_commit.as_deref());
        self.active_diff_view_mut().update(action, &files)
    }

    pub fn update_search(&mut self, action: SearchAction, ctx: &DetailContext<'_>) {
        if ctx.tab == tabs::DetailTab::Commits && self.commits.open_commit.is_none() {
            self.commits.update_search(action);
        } else if self.active_search(ctx.tab).is_some() {
            let files = view::diff_files(ctx.data, self.commits.open_commit.as_deref());
            self.active_diff_view_mut().update_search(action, files);
        }
    }
}

#[cfg(test)]
mod tests;
