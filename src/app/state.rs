use std::collections::{HashMap, HashSet};

use crate::domain::activity::Activity;
use crate::domain::ci::Build;
use crate::domain::comment::Comment;
use crate::domain::commit::Commit;
use crate::domain::diff::Diff;
use crate::domain::pr::{PrStatus, PullRequest};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Description,
    Overview,
    Diff,
    Commits,
    Builds,
}

impl DetailTab {
    pub const ALL: [Self; 5] = [
        Self::Description,
        Self::Overview,
        Self::Diff,
        Self::Commits,
        Self::Builds,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let len = Self::ALL.len();
        Self::ALL[(self.index() + len - 1) % len]
    }
}

#[derive(Debug, Default)]
pub struct AppState {
    pub cache: Cache,
    pub ui: UiMemory,
    pub screen: Screen,
    pub current_user: String,
}

#[derive(Debug, Default)]
pub struct SearchState {
    pub open: bool,
    pub query: String,
}

impl SearchState {
    pub fn matches(&self, haystack: &str) -> bool {
        self.query.is_empty() || haystack.to_lowercase().contains(&self.query.to_lowercase())
    }

    pub fn matches_pr(&self, pr: &PullRequest) -> bool {
        self.matches(&pr.title)
            || self.matches(&pr.author.username)
            || self.matches(&format!("#{}", pr.id))
    }

    fn matches_commit(&self, c: &Commit) -> bool {
        self.matches(&c.oid) || self.matches(&c.headline)
    }

    pub fn filter_commits<'a>(&self, commits: &'a [Commit]) -> Vec<&'a Commit> {
        commits.iter().filter(|c| self.matches_commit(c)).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTarget {
    List,
    Commits,
    DiffTree,
    DiffPane,
}

#[derive(Debug, Default)]
pub struct UiMemory {
    pub list_selected: usize,
    pub list_viewport: u16,
    pub list_filter: StatusFilter,
    pub list_search: SearchState,
    pub filter_picker_open: bool,
    pub filter_picker_cursor: usize,
    pub confirm: Option<ConfirmKind>,
    pub confirm_cursor: usize,
    pub diff: DiffViewState,
    pub commits: CommitsViewState,
    pub description_scroll: u16,
    pub overview_scroll: u16,
    pub overview_cursor: usize,
    pub overview_item_count: usize,
    pub overview_reply: Option<u64>,
    /// The focused thread in Overview, for resolve/unresolve (`R`).
    pub overview_thread: Option<ThreadRef>,
    /// Sub-cursor within the focused block (Ctrl-j/k steps individual comments).
    pub overview_sub: usize,
    pub overview_block_len: usize,
    pub overview_selected: Option<CommentRef>,
    pub description_viewport: u16,
    pub overview_viewport: u16,
    pub help_open: bool,
    pub comment_draft: Option<CommentDraft>,
    pub comment_pending: bool,
    /// A failed action's message, shown as a dismissible popup.
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommentAnchor {
    pub path: String,
    pub line: usize,
    pub removed: bool,
}

#[derive(Debug, Clone)]
pub struct CommentDraft {
    pub target: CommentTarget,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum CommentTarget {
    Line(CommentAnchor),
    Pr,
    Reply(u64),
    /// Editing an existing comment; `review` picks the right provider endpoint.
    Edit { id: u64, review: bool },
}

/// The comment the overview sub-cursor points at within the focused block.
#[derive(Debug, Clone, Copy)]
pub struct CommentRef {
    /// `None` when the provider gave no id (can't edit/delete it).
    pub id: Option<u64>,
    /// A diff/line comment (vs a PR-level one) — GitHub edits them differently.
    pub review: bool,
}

/// Identity of a focused thread, for resolve/unresolve.
#[derive(Debug, Clone)]
pub struct ThreadRef {
    /// GitHub GraphQL thread id.
    pub node_id: Option<String>,
    /// Root comment id (Bitbucket toggles its state).
    pub comment_id: Option<u64>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    Approve,
    DeleteComment { id: u64, review: bool },
}

impl ConfirmKind {
    pub fn prompt(self) -> &'static str {
        match self {
            Self::Approve => "Approve this PR?",
            Self::DeleteComment { .. } => "Delete this comment?",
        }
    }
}

impl UiMemory {
    pub fn active_diff_view(&self) -> &DiffViewState {
        if self.commits.open_commit.is_some() {
            &self.commits.diff
        } else {
            &self.diff
        }
    }

    pub fn active_diff_view_mut(&mut self) -> &mut DiffViewState {
        if self.commits.open_commit.is_some() {
            &mut self.commits.diff
        } else {
            &mut self.diff
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusFilter {
    #[default]
    Open,
    Draft,
    Merged,
    Declined,
    All,
}

impl StatusFilter {
    pub const CYCLE: [Self; 5] = [
        Self::Open,
        Self::Draft,
        Self::Merged,
        Self::Declined,
        Self::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Declined => "Declined",
            Self::All => "All",
        }
    }

    pub fn matches(self, status: &PrStatus) -> bool {
        matches!(
            (self, status),
            (Self::All, _)
                | (Self::Open, PrStatus::Open)
                | (Self::Draft, PrStatus::Draft)
                | (Self::Merged, PrStatus::Merged)
                | (Self::Declined, PrStatus::Declined)
        )
    }
}

#[derive(Debug, Default)]
pub struct DiffViewState {
    pub cursor: usize,
    pub focused_file: usize,
    pub collapsed: HashSet<String>,
    pub pane_scroll: u16,
    pub pane_cursor: usize,
    pub pane_viewport: u16,
    pub tree_viewport: u16,
    pub pane_item_count: usize,
    pub tree_search: SearchState,
    pub pane_search: SearchState,
    pub pane_matches: Vec<usize>,
    pub pane_anchor: Option<CommentAnchor>,
    pub pane_reply: Option<u64>,
    pub pane_thread: Option<ThreadRef>,
    /// Root-comment ids of resolved threads the user has expanded (otherwise
    /// resolved threads render collapsed in the diff).
    pub expanded_threads: HashSet<u64>,
    pub focus: DiffFocus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DiffFocus {
    #[default]
    Tree,
    Pane,
}

#[derive(Debug, Default)]
pub struct CommitsViewState {
    pub selected: usize,
    pub viewport: u16,
    pub search: SearchState,
    pub open_commit: Option<String>,
    pub diff: DiffViewState,
}

#[derive(Debug, Default)]
pub struct Cache {
    pub prs: LoadState<Vec<PullRequest>>,
    pub details: HashMap<u64, PrData>,
}

#[derive(Debug, Default)]
pub struct PrData {
    pub commits: LoadState<Vec<Commit>>,
    pub diff: LoadState<Diff>,
    pub builds: LoadState<Vec<Build>>,
    pub activity: LoadState<Activity>,
    pub commit_diffs: HashMap<String, LoadState<Diff>>,
}

#[derive(Debug, Default)]
pub enum LoadState<T> {
    #[default]
    NotRequested,
    Loading,
    Loaded(T),
    Failed(String),
}

impl<T> LoadState<T> {
    pub fn start_loading(&mut self) -> bool {
        if matches!(self, LoadState::NotRequested | LoadState::Failed(_)) {
            *self = LoadState::Loading;
            true
        } else {
            false
        }
    }

    pub fn from_result(result: Result<T, String>) -> Self {
        match result {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(e),
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, LoadState::Loading)
    }
}

impl PrData {
    fn any_loading(&self) -> bool {
        self.commits.is_loading()
            || self.diff.is_loading()
            || self.builds.is_loading()
            || self.activity.is_loading()
            || self.commit_diffs.values().any(LoadState::is_loading)
    }
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

impl AppState {
    pub fn viewing_own_pr(&self, pr_id: u64) -> bool {
        if self.current_user.is_empty() {
            return false;
        }
        let LoadState::Loaded(prs) = &self.cache.prs else {
            return false;
        };
        prs.iter()
            .any(|pr| pr.id == pr_id && pr.author.username == self.current_user)
    }

    /// The loaded comment with `id` in the current PR's activity, if any.
    pub fn find_comment(&self, id: u64) -> Option<&Comment> {
        let Screen::Detail { pr_id, .. } = self.screen else {
            return None;
        };
        let LoadState::Loaded(activity) = &self.cache.details.get(&pr_id)?.activity else {
            return None;
        };
        activity
            .comments
            .iter()
            .chain(activity.threads.iter().flat_map(|t| t.comments.iter()))
            .find(|c| c.id == Some(id))
    }

    /// The thread the cursor is on, for resolve/unresolve (`R`).
    pub fn focused_thread(&self) -> Option<&ThreadRef> {
        let Screen::Detail { tab, .. } = self.screen else {
            return None;
        };
        match tab {
            DetailTab::Overview => self.ui.overview_thread.as_ref(),
            DetailTab::Diff => self.ui.active_diff_view().pane_thread.as_ref(),
            DetailTab::Commits if self.ui.commits.open_commit.is_some() => {
                self.ui.active_diff_view().pane_thread.as_ref()
            }
            _ => None,
        }
    }

    /// The overview sub-selected comment, but only when it's the viewer's own
    /// (so it can be edited/deleted). `None` otherwise.
    pub fn editable_selected(&self) -> Option<CommentRef> {
        let sel = self.ui.overview_selected?;
        let id = sel.id?;
        let comment = self.find_comment(id)?;
        (!self.current_user.is_empty() && comment.author.username == self.current_user)
            .then_some(sel)
    }

    /// Target for a brand-new comment (`c`): a top-level PR comment in Overview,
    /// or the focused line in the Diff / commit pane.
    pub fn comment_target(&self) -> Option<CommentTarget> {
        let Screen::Detail { tab, .. } = self.screen else {
            return None;
        };
        match tab {
            DetailTab::Overview => Some(CommentTarget::Pr),
            DetailTab::Diff => self.pane_line_target(),
            DetailTab::Commits if self.ui.commits.open_commit.is_some() => self.pane_line_target(),
            _ => None,
        }
    }

    /// Target for a reply (`r`): the focused comment/thread in Overview. `None`
    /// when nothing repliable is focused.
    pub fn reply_target(&self) -> Option<CommentTarget> {
        let Screen::Detail { tab, .. } = self.screen else {
            return None;
        };
        match tab {
            DetailTab::Overview => self.ui.overview_reply.map(CommentTarget::Reply),
            _ => None,
        }
    }

    fn pane_line_target(&self) -> Option<CommentTarget> {
        let view = self.ui.active_diff_view();
        if view.focus != DiffFocus::Pane {
            return None;
        }
        if let Some(parent) = view.pane_reply {
            return Some(CommentTarget::Reply(parent));
        }
        view.pane_anchor.clone().map(CommentTarget::Line)
    }

    pub fn search_target(&self) -> Option<SearchTarget> {
        match self.screen {
            Screen::List => (!self.ui.filter_picker_open).then_some(SearchTarget::List),
            Screen::Detail { tab, .. } => {
                let viewing_commit = self.ui.commits.open_commit.is_some();
                match tab {
                    DetailTab::Commits if !viewing_commit => Some(SearchTarget::Commits),
                    DetailTab::Diff | DetailTab::Commits => {
                        match self.ui.active_diff_view().focus {
                            DiffFocus::Tree => Some(SearchTarget::DiffTree),
                            DiffFocus::Pane => Some(SearchTarget::DiffPane),
                        }
                    }
                    _ => None,
                }
            }
        }
    }

    pub fn filtered_prs(&self) -> Vec<&PullRequest> {
        match &self.cache.prs {
            LoadState::Loaded(prs) => prs
                .iter()
                .filter(|pr| self.ui.list_filter.matches(&pr.status))
                .filter(|pr| self.ui.list_search.matches_pr(pr))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn is_loading(&self) -> bool {
        self.ui.comment_pending
            || self.cache.prs.is_loading()
            || self.cache.details.values().any(PrData::any_loading)
    }
}
