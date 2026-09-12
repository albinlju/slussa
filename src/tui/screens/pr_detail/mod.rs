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
    app::{navigation::Screen, store::LoadState},
    tui::{
        component::Component,
        components::diff_viewer::DiffViewer,
        screens::pr_detail::{dialogs::confirm::ConfirmKind, tabs::commits::CommitList},
    },
};
use ratatui::{Frame, layout::Rect};

pub use view::{DetailContext, DetailView};
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)] // a UI-state bag, not a state machine
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
    pub fn modal_open(&self) -> bool {
        self.confirm.is_some()
            || self.review_picker.is_some()
            || self.merge_picker.is_some()
            || self.help_open
            || self.editor.is_open()
    }

    pub fn active_diff_view(&self) -> &DiffViewer {
        if self.commits.open_commit.is_some() {
            &self.commits.diff
        } else {
            &self.diff
        }
    }

    pub fn active_diff_view_mut(&mut self) -> &mut DiffViewer {
        if self.commits.open_commit.is_some() {
            &mut self.commits.diff
        } else {
            &mut self.diff
        }
    }
}

impl Component for PrDetailScreen {
    type Context<'a> = DetailContext<'a>;
    type Message = crate::app::action::DetailAction;
    fn handle_key(
        &self,
        key: ratatui::crossterm::event::KeyEvent,
        ctx: &DetailContext<'_>,
    ) -> Option<crate::app::action::Action> {
        let view = DetailView {
            detail: self,
            store: ctx.store,
            screen: ctx.screen,
            refreshing: ctx.refreshing,
        };
        keys::key_to_action(&view, key).filter(|action| match action {
            crate::app::action::Action::Detail(action) => view.supports_action(*action),
            _ => true,
        })
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &DetailContext<'_>) {
        render::render(self, frame, area, ctx);
    }
    fn update(
        &mut self,
        action: Self::Message,
        ctx: &DetailContext<'_>,
    ) -> Option<crate::app::action::Action> {
        use crate::app::action::{Action, DetailAction};
        if !(DetailView {
            detail: self,
            store: ctx.store,
            screen: ctx.screen,
            refreshing: ctx.refreshing,
        })
        .supports_action(action)
        {
            return None;
        }
        if let Screen::Detail { pr_id, .. } = ctx.screen
            && ctx.store.operations.contains_key(&pr_id)
            && !matches!(
                action,
                DetailAction::Back
                    | DetailAction::NextTab
                    | DetailAction::PrevTab
                    | DetailAction::SelectTab(_)
                    | DetailAction::ErrorScroll(_)
                    | DetailAction::DismissError
                    | DetailAction::ToggleHelp
                    | DetailAction::BuildsScroll(_)
                    | DetailAction::DescriptionHorizontal(_)
                    | DetailAction::DescriptionScroll(_)
                    | DetailAction::OverviewScroll(_)
                    | DetailAction::OverviewMove(_)
                    | DetailAction::OverviewSubMove(_)
            )
        {
            return None;
        }
        match action {
            DetailAction::Back => {
                self.help_open = false;
                return Some(Action::Navigate(Screen::List));
            }
            DetailAction::NextTab | DetailAction::PrevTab | DetailAction::SelectTab(_) => {
                let Screen::Detail { pr_id, tab } = ctx.screen else {
                    return None;
                };
                let tab = match action {
                    DetailAction::NextTab => tab.step(1, &ctx.store.capabilities),
                    DetailAction::PrevTab => tab.step(-1, &ctx.store.capabilities),
                    DetailAction::SelectTab(tab) => tab,
                    _ => unreachable!(),
                };
                self.active_tab = tab;
                self.commits.open_commit = None;
                return Some(Action::Navigate(Screen::Detail { pr_id, tab }));
            }
            DetailAction::BuildsScroll(_) => {
                self.builds.update(action, &None);
            }
            DetailAction::DescriptionHorizontal(_)
            | DetailAction::DescriptionScroll(_)
            | DetailAction::OverviewScroll(_)
            | DetailAction::OverviewMove(_)
            | DetailAction::OverviewSubMove(_) => {
                let Screen::Detail { pr_id, .. } = ctx.screen else {
                    return None;
                };
                if let LoadState::Loaded(prs) = &ctx.store.cache.prs
                    && let Some(pr) = prs.iter().find(|p| p.id == pr_id)
                {
                    match action {
                        DetailAction::DescriptionHorizontal(_)
                        | DetailAction::DescriptionScroll(_) => {
                            self.description.update(action, &pr);
                        }
                        _ => {
                            self.overview.update(
                                action,
                                &crate::tui::screens::pr_detail::tabs::overview::OverviewContext {
                                    pr,
                                    data: ctx.store.cache.details.get(&pr_id),
                                    capabilities: &ctx.store.capabilities,
                                },
                            );
                        }
                    }
                }
            }
            DetailAction::ErrorScroll(_) => {
                self.error.update(action, &"");
            }
            DetailAction::ToggleHelp => {
                self.help_open = !self.help_open;
                self.help = crate::tui::components::help_dialog::HelpDialog::default();
            }
            DetailAction::CloseConfirm => self.confirm = None,
            DetailAction::ConfirmMove(_) => {
                if let Some(dialog) = &mut self.confirm {
                    return dialog.update(action, &());
                }
            }
            DetailAction::ReviewPreview | DetailAction::ReviewMove(_) => {
                let options = DetailView {
                    detail: self,
                    store: ctx.store,
                    screen: ctx.screen,
                    refreshing: ctx.refreshing,
                }
                .review_context()
                .options;
                let review_ctx = dialogs::review::ReviewContext {
                    options,
                    pending: None,
                };
                if let Some(dialog) = &mut self.review_picker {
                    return dialog.update(action, &review_ctx);
                }
            }
            DetailAction::CloseReviewPicker => self.review_picker = None,
            DetailAction::OpenMergePicker => {
                if !ctx.store.capabilities.merge_strategies.is_empty() {
                    self.merge_picker = Some(dialogs::merge::MergeDialog::default());
                }
            }
            DetailAction::MergeMove(_) => {
                if let Some(dialog) = &mut self.merge_picker {
                    return dialog
                        .update(action, &ctx.store.capabilities.merge_strategies.as_slice());
                }
            }
            DetailAction::CloseMergePicker => self.merge_picker = None,
            DetailAction::OpenDecline => {
                self.confirm = Some(dialogs::confirm::ConfirmDialog::new(ConfirmKind::Decline));
            }
            DetailAction::CommentType(_)
            | DetailAction::CommentDelete
            | DetailAction::CommentMove(_)
            | DetailAction::CommentVertical(_)
            | DetailAction::CommentHome
            | DetailAction::CommentEnd
            | DetailAction::CommentDiscard
            | DetailAction::CommentDiscardConfirm
            | DetailAction::CommentKeep
            | DetailAction::CommentBackspace
            | DetailAction::CommentCancel => {
                self.editor.update(action, &false);
            }
            other => return self.interaction(other, ctx),
        }
        None
    }
}

impl PrDetailScreen {
    pub fn active_search(
        &self,
        tab: tabs::DetailTab,
    ) -> Option<(&crate::tui::components::search_input::SearchInput, bool)> {
        use tabs::DetailTab;
        match tab {
            DetailTab::Commits if self.commits.open_commit.is_none() => {
                Some((&self.commits.search, false))
            }
            DetailTab::Diff | DetailTab::Commits => Some(self.active_diff_view().active_search()),
            _ => None,
        }
    }

    pub fn update_action(
        &mut self,
        action: crate::app::action::Action,
        ctx: &DetailContext<'_>,
    ) -> Option<crate::app::action::Action> {
        use crate::{app::action::Action, tui::components::diff_viewer::DiffContext};
        let Screen::Detail { pr_id, tab } = ctx.screen else {
            return None;
        };
        let data = ctx.store.cache.details.get(&pr_id);
        match action {
            Action::Detail(action) => self.update(action, ctx),
            Action::Commits(action) => self.commits.update(
                action,
                &tabs::commits::CommitContext {
                    pr_id,
                    data,
                    pending: &[],
                    author: "",
                },
            ),
            Action::Search(action)
                if tab == tabs::DetailTab::Commits && self.commits.open_commit.is_none() =>
            {
                self.commits.update_search(action);
                None
            }
            Action::Diff(_) | Action::Search(_) => {
                let diff = data.and_then(|d| d.diff_for(self.commits.open_commit.as_deref()));
                let diff_ctx = DiffContext {
                    diff,
                    threads: &[],
                    pending: &[],
                    author: "",
                };
                match action {
                    Action::Diff(action) => self.active_diff_view_mut().update(action, &diff_ctx),
                    Action::Search(action) => {
                        if self.active_search(tab).is_some() {
                            self.active_diff_view_mut().update_search(action, &diff_ctx);
                        }
                        None
                    }
                    _ => unreachable!(),
                }
            }
            effect => Some(effect),
        }
    }
}
