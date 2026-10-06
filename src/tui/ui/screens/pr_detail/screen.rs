use super::{DetailContext, dialogs, keys, render, tabs, view};
use crate::{
    domain::pr::PrId,
    tui::{
        app::{effect::Effect, navigation::Screen},
        ui::{
            action::{
                Action, CommitsAction, DetailAction, DiffAction, EditorAction, ErrorAction,
                NavAction, SearchAction,
            },
            component::Component,
            components::{
                diff_viewer::DiffViewer,
                help_dialog::HelpDialog,
                search_input::{SearchInput, SearchKind},
            },
            screens::pr_detail::{
                dialogs::{
                    confirm::ConfirmDialog, issues::IssueDialog, merge::MergeDialog,
                    review::ReviewDialog,
                },
                tabs::commits::CommitList,
            },
        },
    },
};
use ratatui::{Frame, layout::Rect};

/// What the content area shows: the tab, and on Commits whether a commit is
/// open. The keys, the footer and the comment targets all ask this, so they
/// cannot disagree about which diff is on screen, if any.
#[derive(Debug, Clone, Copy)]
pub enum Surface<'a> {
    Description,
    Overview,
    /// The PR's diff.
    Diff(&'a DiffViewer),
    CommitList,
    /// The diff of one commit.
    CommitDiff(&'a DiffViewer),
    Builds,
    /// The log of one build.
    BuildLog,
}

impl<'a> Surface<'a> {
    /// The diff viewer on screen, if a diff is what is shown.
    pub const fn diff_viewer(self) -> Option<&'a DiffViewer> {
        match self {
            Self::Diff(viewer) | Self::CommitDiff(viewer) => Some(viewer),
            Self::Description
            | Self::Overview
            | Self::CommitList
            | Self::Builds
            | Self::BuildLog => None,
        }
    }
}
/// The dialog in front of the PR screen. There is one or none: opening one
/// replaces whatever was there, so two can never be open at once and the keys
/// and the drawing cannot disagree about which is on top.
#[derive(Debug)]
pub enum Overlay {
    Help(HelpDialog),
    Confirm(ConfirmDialog),
    Review(ReviewDialog),
    Merge(MergeDialog),
    Issues(IssueDialog),
}

#[derive(Debug, Default)]
pub struct PrDetailScreen {
    pub overview: tabs::overview::Overview,
    pub builds: tabs::builds::Builds,
    pub description: tabs::description::Description,
    pub overlay: Option<Overlay>,
    pub diff: DiffViewer,
    pub commits: CommitList,
    pub error: dialogs::error::ErrorDialog,
    pub editor: crate::tui::ui::components::comment_editor::CommentEditor,
    pub(super) pr_id: Option<PrId>,
    pub active_tab: tabs::DetailTab,
    pub(super) navigation: std::collections::HashMap<PrId, DetailNavigation>,
    pub(super) editors:
        std::collections::HashMap<PrId, crate::tui::ui::components::comment_editor::CommentEditor>,
}

/// Where the user was in a PR, kept while another PR is open.
#[derive(Debug, Default)]
pub(super) struct DetailNavigation {
    pub(super) overview: tabs::overview::Overview,
    pub(super) builds: tabs::builds::Builds,
    pub(super) description: tabs::description::Description,
    pub(super) diff: DiffViewer,
    pub(super) commits: CommitList,
    pub(super) tab: tabs::DetailTab,
}

impl PrDetailScreen {
    pub fn reconcile_commits(
        &mut self,
        pr_id: PrId,
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
        self.overlay.is_some() || self.editor.is_open()
    }

    pub const fn help_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Help(_)))
    }

    pub const fn confirm(&self) -> Option<&ConfirmDialog> {
        match &self.overlay {
            Some(Overlay::Confirm(dialog)) => Some(dialog),
            Some(
                Overlay::Help(_) | Overlay::Review(_) | Overlay::Merge(_) | Overlay::Issues(_),
            )
            | None => None,
        }
    }

    pub const fn review_picker(&self) -> Option<&ReviewDialog> {
        match &self.overlay {
            Some(Overlay::Review(dialog)) => Some(dialog),
            Some(
                Overlay::Help(_) | Overlay::Confirm(_) | Overlay::Merge(_) | Overlay::Issues(_),
            )
            | None => None,
        }
    }

    pub const fn merge_picker(&self) -> Option<&MergeDialog> {
        match &self.overlay {
            Some(Overlay::Merge(dialog)) => Some(dialog),
            Some(
                Overlay::Help(_) | Overlay::Confirm(_) | Overlay::Review(_) | Overlay::Issues(_),
            )
            | None => None,
        }
    }

    pub const fn issue_picker(&self) -> Option<&IssueDialog> {
        match &self.overlay {
            Some(Overlay::Issues(dialog)) => Some(dialog),
            Some(
                Overlay::Help(_) | Overlay::Confirm(_) | Overlay::Review(_) | Overlay::Merge(_),
            )
            | None => None,
        }
    }

    /// What the content area shows on `tab`.
    pub const fn surface(&self, tab: tabs::DetailTab) -> Surface<'_> {
        use tabs::DetailTab;
        match tab {
            DetailTab::Description => Surface::Description,
            DetailTab::Overview => Surface::Overview,
            DetailTab::Diff => Surface::Diff(&self.diff),
            DetailTab::Commits => match self.commits.diff() {
                Some(viewer) => Surface::CommitDiff(viewer),
                None => Surface::CommitList,
            },
            DetailTab::Builds if self.builds.log_open() => Surface::BuildLog,
            DetailTab::Builds => Surface::Builds,
        }
    }

    /// The diff viewer on screen, to change it.
    const fn diff_viewer_mut(&mut self, tab: tabs::DetailTab) -> Option<&mut DiffViewer> {
        use tabs::DetailTab;
        match tab {
            DetailTab::Diff => Some(&mut self.diff),
            DetailTab::Commits => self.commits.diff_mut(),
            DetailTab::Description | DetailTab::Overview | DetailTab::Builds => None,
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
            DetailAction::Builds(action) => {
                return self
                    .builds
                    .update(action, &tabs::builds::BuildsInput::new(pr_id, ctx.data));
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
            DetailAction::Issues(action) => return self.issue_action(action, ctx),
            DetailAction::Editor(EditorAction::Submit) => return self.submit_editor(pr_id, ctx),
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
        pr_id: PrId,
        tab: tabs::DetailTab,
        ctx: &DetailContext<'_>,
    ) -> Option<Effect> {
        let tab = match action {
            NavAction::Back => {
                if self.help_open() {
                    self.overlay = None;
                }
                return Some(Effect::Navigate(Screen::List));
            }
            NavAction::ToggleHelp => {
                self.overlay = if self.help_open() {
                    None
                } else {
                    Some(Overlay::Help(HelpDialog::default()))
                };
                return None;
            }
            NavAction::NextTab => tab.step(1, &ctx.store.capabilities),
            NavAction::PrevTab => tab.step(-1, &ctx.store.capabilities),
            NavAction::SelectTab(tab) => tab,
        };
        self.active_tab = tab;
        self.commits.close_commit();
        self.builds.close_log();
        Some(Effect::Navigate(Screen::Detail { pr_id, tab }))
    }

    pub const fn active_search(&self, tab: tabs::DetailTab) -> Option<(&SearchInput, SearchKind)> {
        match self.surface(tab) {
            Surface::CommitList => Some((&self.commits.search, SearchKind::Filter)),
            Surface::Diff(viewer) | Surface::CommitDiff(viewer) => Some(viewer.active_search()),
            Surface::Description | Surface::Overview | Surface::Builds | Surface::BuildLog => None,
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
        let files = view::diff_files(ctx.data, self.commits.open_commit());
        self.diff_viewer_mut(ctx.tab)?.update(action, &files)
    }

    pub fn update_search(&mut self, action: SearchAction, ctx: &DetailContext<'_>) {
        let files = view::diff_files(ctx.data, self.commits.open_commit());
        match self.diff_viewer_mut(ctx.tab) {
            Some(viewer) => viewer.update_search(action, files),
            None if ctx.tab == tabs::DetailTab::Commits => self.commits.update_search(action),
            None => {}
        }
    }
}
