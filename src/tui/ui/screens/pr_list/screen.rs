//! The list's own state and how keys change it.
use super::{Sort, StatusFilter};
use crate::{
    domain::{
        attention::attention,
        pr::{PrGroup, PullRequest},
        seen::Seen,
        user::Username,
    },
    tui::{
        app::{
            effect::Effect,
            navigation::Screen,
            pr_groups::OpenChain,
            store::{LoadState, Store},
        },
        ui::{
            action::{Action, ListAction},
            component::{Component, step_index},
            components::{
                help_dialog::HelpDialog,
                search_input::{SearchInput, SearchKind},
            },
            screens::half_page,
        },
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    widgets::ListState,
};

/// What is in front of the list. One or none: opening one replaces the other.
#[derive(Debug)]
pub enum ListOverlay {
    Help(HelpDialog),
    /// The filter picker, on the filter Enter would apply.
    FilterPicker {
        highlighted: StatusFilter,
    },
}

#[derive(Debug, Default)]
pub struct PrListScreen {
    pub overlay: Option<ListOverlay>,
    pub selected: usize,
    pub(super) list_state: ListState,
    pub viewport: u16,
    pub filter: StatusFilter,
    pub search: SearchInput,
    pub sort: Sort,
}

#[expect(clippy::struct_excessive_bools, reason = "independent loading facts")]
pub struct ListContext<'a> {
    pub prs: &'a LoadState<Vec<PullRequest>>,
    pub refreshing: bool,
    /// Who is looking, for the attention column and order.
    pub viewer: &'a Username,
    /// When each PR was last looked at, to mark the ones changed since.
    pub seen: &'a Seen,
    /// More PRs exist, in a group this view shows, that have not been loaded.
    pub more: bool,
    /// The open PRs are being read page by page.
    pub loading_more: bool,
    /// A group this view shows is being read.
    pub view_loading: bool,
    /// Pages of open PRs are still being appended. The rows stay in the order
    /// they arrive, so nothing moves under the reader; the attention order is
    /// applied once, when the reading ends.
    pub arriving: bool,
}

impl<'a> ListContext<'a> {
    pub fn from_store(store: &'a Store, filter: StatusFilter, screen: Screen) -> Self {
        let groups = filter.groups();
        let arriving = matches!(store.open_chain, OpenChain::Appending);
        Self {
            prs: &store.cache.prs,
            refreshing: store.refreshing(screen),
            viewer: &store.current_user,
            seen: &store.seen,
            more: groups.iter().any(|&group| store.group_has_more(group)),
            loading_more: groups.contains(&PrGroup::Open) && arriving,
            view_loading: groups.iter().any(|&group| store.group_loading(group)),
            arriving,
        }
    }
}

impl Component for PrListScreen {
    type Input<'a> = ListContext<'a>;
    type View<'a> = ListContext<'a>;
    type Message = ListAction;

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &ListContext<'_>) {
        super::render::render(self, frame, area, ctx);
    }

    fn handle_key(&self, key: KeyEvent, ctx: &ListContext<'_>) -> Option<Action> {
        match &self.overlay {
            Some(ListOverlay::FilterPicker { .. }) => {
                return match key.code {
                    KeyCode::Char('q') => Some(Action::Effect(Effect::Quit)),
                    KeyCode::Esc | KeyCode::Char('f') => {
                        Some(Action::List(ListAction::CloseFilterPicker))
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        Some(Action::List(ListAction::FilterPickerNext))
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        Some(Action::List(ListAction::FilterPickerPrev))
                    }
                    KeyCode::Enter => Some(Action::List(ListAction::ApplyFilter)),
                    _ => None,
                };
            }
            Some(ListOverlay::Help(help)) => {
                return match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => Some(Action::List(ListAction::ToggleHelp)),
                    KeyCode::Char('q') => Some(Action::Effect(Effect::Quit)),
                    _ => help.handle_key(key, &()),
                };
            }
            None => {}
        }
        if key.modifiers.is_empty() {
            let kind = match key.code {
                KeyCode::Char('o') => Some(crate::tui::app::effect::LinkAction::Open),
                KeyCode::Char('y') => Some(crate::tui::app::effect::LinkAction::Copy),
                _ => None,
            };
            if let Some(kind) = kind {
                return self
                    .filtered_prs(ctx)
                    .get(self.selected)
                    .filter(|pr| pr.url.is_some())
                    .map(|pr| Action::Effect(Effect::PrLink { pr_id: pr.id, kind }));
            }
        }
        let half = half_page(self.viewport);
        match key.code {
            KeyCode::Char('q') => Some(Action::Effect(Effect::Quit)),
            KeyCode::Char('?') => Some(Action::List(ListAction::ToggleHelp)),
            KeyCode::Char('F') => Some(Action::Effect(Effect::Refresh)),
            KeyCode::Char('f') => Some(Action::List(ListAction::OpenFilterPicker)),
            KeyCode::Char('s') => Some(Action::List(ListAction::ToggleSort)),
            KeyCode::Char('L') if Self::can_load_older(ctx) => {
                Some(Action::List(ListAction::LoadOlder))
            }
            KeyCode::Down | KeyCode::Char('j') => Some(Action::List(ListAction::MoveSelection(1))),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::List(ListAction::MoveSelection(-1))),
            KeyCode::PageDown => Some(Action::List(ListAction::MoveSelection(half))),
            KeyCode::PageUp => Some(Action::List(ListAction::MoveSelection(-half))),
            KeyCode::Enter => {
                let rows = self.filtered_prs(ctx);
                // `#44` in the search names a PR: that one when it is in the
                // list, and when nothing else matches, one the list does not hold.
                self.search
                    .pr_number()
                    .filter(|id| rows.is_empty() || rows.iter().any(|pr| pr.id == *id))
                    .or_else(|| rows.get(self.selected).map(|pr| pr.id))
                    .map(|id| Action::List(ListAction::OpenPr(id)))
            }
            _ => None,
        }
    }

    fn update(&mut self, action: ListAction, ctx: &ListContext<'_>) -> Option<Effect> {
        match action {
            ListAction::ToggleHelp => {
                self.overlay = if matches!(self.overlay, Some(ListOverlay::Help(_))) {
                    None
                } else {
                    Some(ListOverlay::Help(HelpDialog::default()))
                };
            }
            ListAction::ToggleSort => {
                // Follow the highlighted PR to its new place.
                let current = self.filtered_prs(ctx).get(self.selected).map(|pr| pr.id);
                self.sort = self.sort.toggled();
                let index =
                    current.and_then(|id| self.filtered_prs(ctx).iter().position(|pr| pr.id == id));
                self.selected = index.unwrap_or(0);
            }
            ListAction::MoveSelection(delta) => {
                self.selected = step_index(self.selected, delta, self.filtered_prs(ctx).len());
            }
            ListAction::OpenPr(id) => return Some(Effect::OpenPr(id)),
            ListAction::LoadOlder => return Some(Effect::LoadOlder),
            ListAction::OpenFilterPicker => {
                self.overlay = Some(ListOverlay::FilterPicker {
                    highlighted: self.filter,
                });
            }
            ListAction::CloseFilterPicker => {
                if matches!(self.overlay, Some(ListOverlay::FilterPicker { .. })) {
                    self.overlay = None;
                }
            }
            ListAction::FilterPickerNext => self.step_filter_picker(StatusFilter::next),
            ListAction::FilterPickerPrev => self.step_filter_picker(StatusFilter::previous),
            ListAction::ApplyFilter => {
                if let Some(ListOverlay::FilterPicker { highlighted }) = self.overlay {
                    self.overlay = None;
                    if highlighted != self.filter {
                        self.filter = highlighted;
                        self.selected = 0;
                        self.list_state = ListState::default();
                        return Some(Effect::LoadView);
                    }
                }
            }
        }
        None
    }
}

impl PrListScreen {
    /// `L` is offered only where it would show something: while a group this
    /// view shows has more PRs unread, and nothing is being read.
    pub(super) const fn can_load_older(ctx: &ListContext<'_>) -> bool {
        ctx.more && !ctx.loading_more
    }

    /// The rows to show, filtered and ordered. With the attention order the
    /// PRs that need the viewer come first, most urgent first; the sort is
    /// stable, so the provider's order holds within each group.
    pub fn filtered_prs<'a>(&self, ctx: &ListContext<'a>) -> Vec<&'a PullRequest> {
        let LoadState::Loaded(prs) = ctx.prs else {
            return Vec::new();
        };
        let mut rows: Vec<&PullRequest> = prs
            .iter()
            .filter(|p| self.filter.matches(&p.status))
            .filter(|p| self.search.matches_pr(p))
            .collect();
        if self.sort == Sort::Attention && !ctx.arriving {
            rows.sort_by_key(|pr| {
                attention(pr, ctx.viewer).map_or(usize::MAX, |reason| reason as usize)
            });
        }
        rows
    }

    fn step_filter_picker(&mut self, step: impl FnOnce(StatusFilter) -> StatusFilter) {
        if let Some(ListOverlay::FilterPicker { highlighted }) = &mut self.overlay {
            *highlighted = step(*highlighted);
        }
    }

    pub fn update_search(&mut self, action: crate::tui::ui::action::SearchAction) {
        use crate::tui::ui::action::SearchAction;
        self.search.update(action, &SearchKind::Filter);
        if !matches!(action, SearchAction::Open | SearchAction::Confirm) {
            self.selected = 0;
            self.list_state = ListState::default();
        }
    }
}
