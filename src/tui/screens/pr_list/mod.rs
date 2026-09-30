use crate::{
    app::{
        action::{Action, ListAction},
        store::LoadState,
    },
    domain::{
        attention::{Attention, attention},
        ci::CiSummary,
        pr::{PrStatus, PullRequest},
        review::{Reviewer, ReviewerState},
    },
    tui::{
        component::{Component, step_index},
        components::search_input::SearchInput,
        icons, layout,
        screens::half_page,
        theme,
        widgets::{
            self,
            table::{self, Cell, Column, Width},
        },
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

const GUTTER: u16 = 2;
const HELP_KEYS: &[(&str, &str)] = &[
    ("j/k / ↑↓", "move up/down"),
    ("enter", "open PR"),
    ("o", "open PR in browser"),
    ("y", "copy PR link"),
    ("/", "search title / author"),
    ("esc", "clear search"),
    ("f", "filter status"),
    ("s", "sort: needs you first / newest first"),
    ("^d/^u", "half-page"),
    ("F", "refresh"),
    ("? / esc", "close help"),
    ("q", "quit"),
];

// Reserve space for the title first; secondary details remain in the PR view.
// Index 9 is the attention column; `render` drops it while no row has a reason.
const ATTENTION_COLUMN: usize = 9;
const fn visible_columns(width: u16) -> &'static [usize] {
    match width {
        0..=59 => &[0, 3],
        60..=89 => &[0, 3, 2, 4],
        90..=119 => &[0, 3, 9, 2, 4, 7, 8],
        _ => &[0, 3, 9, 2, 1, 4, 5, 6, 7, 8],
    }
}

const COLS: &[Column] = &[
    Column {
        title: "#",
        width: Width::Fixed(7),
    },
    Column {
        title: "Status",
        width: Width::Fixed(10),
    },
    Column {
        title: "Author",
        width: Width::Fixed(18),
    },
    Column {
        title: "Title",
        width: Width::Flex(1),
    },
    Column {
        title: "CI",
        width: Width::Fixed(4),
    },
    Column {
        title: "Diff",
        width: Width::Fixed(12),
    },
    Column {
        title: "Comments",
        width: Width::Fixed(10),
    },
    Column {
        title: "Reviews",
        width: Width::Fixed(9),
    },
    Column {
        title: "Age",
        width: Width::Fixed(8),
    },
    Column {
        title: "Needs you",
        width: Width::Fixed(19),
    },
];

fn pr_list_container(filter: StatusFilter, count_label: &str) -> Block<'static> {
    let theme = theme::current();
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Line::styled(
            format!(" {} ({count_label}) ", filter.label()),
            Style::default()
                .fg(theme.orange)
                .add_modifier(Modifier::BOLD),
        ))
}

fn render_table_header(frame: &mut Frame<'_>, table: &table::Table<'_>, area: Rect) {
    let mut header = table.header();
    header
        .spans
        .insert(0, Span::raw(" ".repeat(GUTTER as usize)));
    frame.render_widget(Paragraph::new(header), area);
}

fn render_table_body(
    frame: &mut Frame<'_>,
    table: &table::Table<'_>,
    prs: &[&PullRequest],
    list_state: &mut ListState,
    area: Rect,
    columns: &[usize],
    viewer: &str,
) {
    let theme = theme::current();
    let items: Vec<ListItem<'_>> = prs
        .iter()
        .map(|pr| {
            let cells = row_cells(pr, viewer);
            ListItem::new(
                table.row(
                    &columns
                        .iter()
                        .map(|&i| cells[i].clone())
                        .collect::<Vec<_>>(),
                ),
            )
        })
        .collect();
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, list_state);
}

fn render_footer(
    frame: &mut Frame<'_>,
    search: &SearchInput,
    match_count: usize,
    refreshing: bool,
    area: Rect,
) {
    let line = if search.open {
        widgets::search_prompt(&search.query, match_count, area.width)
    } else {
        widgets::footer(
            area.width,
            &widgets::hints_on("enter: open  /: search  f: filter"),
            refreshing,
        )
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_filter_picker(frame: &mut Frame<'_>, state: &PrListScreen, area: Rect) {
    let theme = theme::current();
    let list_area = widgets::dialog::frame(
        frame,
        area,
        "Filter",
        (44, StatusFilter::CYCLE.len() as u16),
        &[("j/k", "move"), ("Enter", "select"), ("Esc", "cancel")],
    );
    let items: Vec<ListItem<'_>> = StatusFilter::CYCLE
        .iter()
        .map(|f| ListItem::new(Line::raw(f.label())))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(state.filter_picker_cursor));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, list_area, &mut list_state);
}

fn attention_cell(reason: Option<Attention>) -> Cell {
    let theme = theme::current();
    reason.map_or_else(Vec::new, |reason| {
        let color = match reason {
            Attention::ChangesRequested | Attention::CiFailed => theme.error,
            Attention::ReviewRequested => theme.warning,
            Attention::Approved => theme.success,
        };
        vec![Span::styled(reason.label(), Style::default().fg(color))]
    })
}

fn row_cells(pr: &PullRequest, viewer: &str) -> Vec<Cell> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);

    let age = age_label(pr.created);

    let (ci_sym, ci_color) = match pr.ci {
        CiSummary::Success => (icons::CHECK_CIRCLE, theme.success),
        CiSummary::Failed => (icons::TIMES_CIRCLE, theme.error),
        CiSummary::Pending => (icons::CLOCK, theme.warning),
        // adjust — half circle, neutral/not run
        CiSummary::Unknown => (icons::ADJUST, theme.muted),
    };

    let comm_text = pr.comment_count.to_string();

    let (rev_text, rev_color) = review_summary(&pr.reviewers);

    vec![
        vec![Span::styled(format!("#{}", pr.id), muted)],
        vec![Span::styled(
            pr.status.label().to_string(),
            Style::default().fg(theme.status_color(&pr.status)),
        )],
        vec![Span::styled(
            pr.author.username.clone(),
            Style::default().fg(theme.info),
        )],
        vec![Span::styled(
            pr.title.clone(),
            Style::default().fg(theme.fg),
        )],
        vec![Span::styled(ci_sym, Style::default().fg(ci_color))],
        vec![
            Span::styled(
                format!("+{}", pr.additions),
                Style::default().fg(theme.diff_added),
            ),
            Span::raw(" "),
            Span::styled(
                format!("-{}", pr.deletions),
                Style::default().fg(theme.diff_removed),
            ),
        ],
        vec![Span::styled(comm_text, muted)],
        vec![Span::styled(rev_text, Style::default().fg(rev_color))],
        vec![Span::styled(age, muted)],
        attention_cell(attention(pr, viewer)),
    ]
}

fn age_label(created: DateTime<Utc>) -> String {
    match (Utc::now() - created).num_days() {
        0 => "today".to_string(),
        1 => "1d".to_string(),
        days => format!("{days}d"),
    }
}

fn review_summary(reviewers: &[Reviewer]) -> (String, Color) {
    let theme = theme::current();
    if reviewers.is_empty() {
        return ("—".to_string(), theme.muted);
    }
    let approved = reviewers
        .iter()
        .filter(|r| r.state == ReviewerState::Approved)
        .count();
    let total = reviewers.len();
    let any_blocking = reviewers
        .iter()
        .any(|r| r.state == ReviewerState::ChangesRequested);
    let color = if any_blocking {
        theme.error
    } else if approved == total {
        theme.success
    } else {
        theme.warning
    };
    (format!("{approved}/{total}"), color)
}

#[derive(Debug, Default)]
pub struct PrListScreen {
    pub help_open: bool,
    pub help: crate::tui::components::help_dialog::HelpDialog,
    pub selected: usize,
    list_state: ListState,
    pub viewport: u16,
    pub filter: StatusFilter,
    pub search: SearchInput,
    pub filter_picker_open: bool,
    pub filter_picker_cursor: usize,
    pub sort: Sort,
}

/// How the list is ordered. Both keep the provider's order (newest first)
/// within a group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    /// PRs that need the viewer first, most urgent first, then the rest.
    #[default]
    Attention,
    /// The provider's order only.
    Recent,
}

impl Sort {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Attention => Self::Recent,
            Self::Recent => Self::Attention,
        }
    }

    /// The value written in `config.toml`; unset or unknown means attention.
    pub fn from_config(name: Option<&str>) -> Self {
        match name {
            None | Some("attention") => Self::Attention,
            Some("recent") => Self::Recent,
            Some(other) => {
                tracing::warn!("unknown sort {other:?}, using attention");
                Self::Attention
            }
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

    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Declined => "Declined",
            Self::All => "All",
        }
    }

    pub const fn matches(self, status: &PrStatus) -> bool {
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

pub struct ListContext<'a> {
    pub prs: &'a LoadState<Vec<PullRequest>>,
    pub refreshing: bool,
    /// Who is looking, for the attention column and order.
    pub viewer: &'a str,
}

impl Component for PrListScreen {
    type Context<'a> = ListContext<'a>;
    type Message = ListAction;
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &ListContext<'_>) {
        self.viewport = area.height.saturating_sub(4);

        let [body_area, footer_area] = layout::split(
            area,
            Direction::Vertical,
            [Constraint::Min(0), Constraint::Length(1)],
        );

        let filtered =
            matches!(ctx.prs, LoadState::Loaded(_)).then(|| self.filtered_prs(ctx.prs, ctx.viewer));
        let count_label = match (&filtered, ctx.prs) {
            (Some(prs), _) => prs.len().to_string(),
            (_, LoadState::Failed(_)) => "!".to_string(),
            _ => "…".to_string(),
        };

        let container = pr_list_container(self.filter, &count_label);
        let inner = container.inner(body_area);
        frame.render_widget(container, body_area);

        let [header_area, rows_area] = layout::split(
            inner,
            Direction::Vertical,
            [Constraint::Length(1), Constraint::Min(0)],
        );
        let width = inner.width.saturating_sub(GUTTER);
        let any_reason = filtered
            .as_ref()
            .is_some_and(|prs| prs.iter().any(|pr| attention(pr, ctx.viewer).is_some()));
        let columns: Vec<usize> = visible_columns(width)
            .iter()
            .copied()
            .filter(|&i| i != ATTENTION_COLUMN || any_reason)
            .collect();
        let columns = columns.as_slice();
        let definitions: Vec<_> = columns
            .iter()
            .map(|&i| Column {
                title: COLS[i].title,
                width: COLS[i].width,
            })
            .collect();
        let table = table::Table::new(&definitions, width);
        render_table_header(frame, &table, header_area);

        if let Some(prs) = &filtered {
            if prs.is_empty() {
                let message = if self.search.query.is_empty() {
                    "No PRs in this view. f changes filter; F refreshes."
                } else {
                    "No matching PRs. Esc clears search; f changes filter."
                };
                frame.render_widget(widgets::empty_state(message), rows_area);
            } else {
                self.list_state.select(Some(self.selected));
                render_table_body(
                    frame,
                    &table,
                    prs,
                    &mut self.list_state,
                    rows_area,
                    columns,
                    ctx.viewer,
                );
            }
        } else {
            widgets::loaded_or_placeholder(frame, Some(ctx.prs), "pull requests", rows_area);
        }

        let match_count = filtered.as_ref().map_or(0, Vec::len);
        render_footer(
            frame,
            &self.search,
            match_count,
            ctx.refreshing,
            footer_area,
        );

        if self.help_open {
            let has_link = self
                .filtered_prs(ctx.prs, ctx.viewer)
                .get(self.selected)
                .is_some_and(|pr| pr.url.is_some());
            let entries: Vec<_> = HELP_KEYS
                .iter()
                .copied()
                .filter(|(key, _)| has_link || !matches!(*key, "o" | "y"))
                .collect();
            self.help.render(frame, area, &entries.as_slice());
        }
        if self.filter_picker_open {
            render_filter_picker(frame, self, area);
        }
    }
    fn handle_key(&self, key: KeyEvent, ctx: &ListContext<'_>) -> Option<Action> {
        if self.filter_picker_open {
            return match key.code {
                KeyCode::Char('q') => Some(Action::Quit),
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

        if self.help_open {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('?') => Some(Action::List(ListAction::ToggleHelp)),
                KeyCode::Char('q') => Some(Action::Quit),
                _ => self.help.handle_key(key, &HELP_KEYS),
            };
        }
        if key.modifiers.is_empty() {
            let kind = match key.code {
                KeyCode::Char('o') => Some(crate::app::action::LinkAction::Open),
                KeyCode::Char('y') => Some(crate::app::action::LinkAction::Copy),
                _ => None,
            };
            if let Some(kind) = kind {
                return self
                    .filtered_prs(ctx.prs, ctx.viewer)
                    .get(self.selected)
                    .filter(|pr| pr.url.is_some())
                    .map(|pr| Action::PrLink { pr_id: pr.id, kind });
            }
        }
        let half = half_page(self.viewport);
        match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('?') => Some(Action::List(ListAction::ToggleHelp)),
            KeyCode::Char('F') => Some(Action::Refresh),
            KeyCode::Char('f') => Some(Action::List(ListAction::OpenFilterPicker)),
            KeyCode::Char('s') => Some(Action::List(ListAction::ToggleSort)),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::List(ListAction::MoveSelection(1))),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::List(ListAction::MoveSelection(-1))),
            KeyCode::PageDown => Some(Action::List(ListAction::MoveSelection(half))),
            KeyCode::PageUp => Some(Action::List(ListAction::MoveSelection(-half))),
            KeyCode::Enter => self
                .filtered_prs(ctx.prs, ctx.viewer)
                .get(self.selected)
                .map(|p| Action::List(ListAction::OpenPr(p.id))),
            _ => None,
        }
    }

    fn update(&mut self, action: ListAction, ctx: &ListContext<'_>) -> Option<Action> {
        match action {
            ListAction::ToggleHelp => {
                self.help_open = !self.help_open;
                self.help = crate::tui::components::help_dialog::HelpDialog::default();
            }
            ListAction::ToggleSort => {
                // Follow the highlighted PR to its new place.
                let current = self
                    .filtered_prs(ctx.prs, ctx.viewer)
                    .get(self.selected)
                    .map(|pr| pr.id);
                self.sort = self.sort.toggled();
                let index = current.and_then(|id| {
                    self.filtered_prs(ctx.prs, ctx.viewer)
                        .iter()
                        .position(|pr| pr.id == id)
                });
                self.selected = index.unwrap_or(0);
            }
            ListAction::MoveSelection(delta) => {
                self.selected = step_index(
                    self.selected,
                    delta,
                    self.filtered_prs(ctx.prs, ctx.viewer).len(),
                );
            }
            ListAction::OpenPr(id) => return Some(Action::List(ListAction::OpenPr(id))),
            ListAction::OpenFilterPicker => self.open_filter_picker(),
            ListAction::CloseFilterPicker => self.close_filter_picker(),
            ListAction::FilterPickerNext => self.filter_picker_next(),
            ListAction::FilterPickerPrev => self.filter_picker_prev(),
            ListAction::ApplyFilter => self.apply_filter(),
        }
        None
    }
}
impl PrListScreen {
    /// The rows to show, filtered and ordered. With the attention order the
    /// PRs that need `viewer` come first, most urgent first; the sort is stable,
    /// so the provider's order holds within each group.
    pub fn filtered_prs<'a>(
        &self,
        prs: &'a LoadState<Vec<PullRequest>>,
        viewer: &str,
    ) -> Vec<&'a PullRequest> {
        let LoadState::Loaded(prs) = prs else {
            return Vec::new();
        };
        let mut rows: Vec<&PullRequest> = prs
            .iter()
            .filter(|p| self.filter.matches(&p.status))
            .filter(|p| self.search.matches_pr(p))
            .collect();
        if self.sort == Sort::Attention {
            rows.sort_by_key(|pr| {
                attention(pr, viewer).map_or(usize::MAX, |reason| reason as usize)
            });
        }
        rows
    }
    fn open_filter_picker(&mut self) {
        self.filter_picker_cursor = StatusFilter::CYCLE
            .iter()
            .position(|&f| f == self.filter)
            .unwrap_or(0);
        self.filter_picker_open = true;
    }

    const fn close_filter_picker(&mut self) {
        self.filter_picker_open = false;
    }

    fn filter_picker_next(&mut self) {
        let last = StatusFilter::CYCLE.len().saturating_sub(1);
        self.filter_picker_cursor = (self.filter_picker_cursor + 1).min(last);
    }

    const fn filter_picker_prev(&mut self) {
        self.filter_picker_cursor = self.filter_picker_cursor.saturating_sub(1);
    }

    fn apply_filter(&mut self) {
        let new_filter = StatusFilter::CYCLE
            .get(self.filter_picker_cursor)
            .copied()
            .unwrap_or(StatusFilter::Open);
        if new_filter != self.filter {
            self.filter = new_filter;
            self.selected = 0;
            self.list_state = ListState::default();
        }
        self.filter_picker_open = false;
    }
}

impl PrListScreen {
    pub fn update_search(&mut self, action: crate::app::action::SearchAction) {
        use crate::app::action::SearchAction;
        self.search.update(
            action,
            &crate::tui::components::search_input::SearchContext {
                highlight: false,
                matches: 0,
            },
        );
        if !matches!(action, SearchAction::Open | SearchAction::Confirm) {
            self.selected = 0;
            self.list_state = ListState::default();
        }
    }
}

#[cfg(test)]
mod tests;
