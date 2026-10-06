use super::bindings;
use crate::{
    domain::{diff::FileDiff, review::CommentTarget},
    tui::{
        app::store::{LoadState, PrData},
        ui::{
            components::{diff_viewer::DiffFocus, search_input::SearchInput},
            screens::pr_detail::{
                DetailView, Surface,
                tabs::{DetailTab, builds::BuildsInput},
            },
            theme,
            widgets::{self, Hint},
        },
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

pub(super) fn render(frame: &mut Frame<'_>, state: &DetailView<'_>, area: Rect) {
    let (pr_data, tab) = (state.data, state.tab);
    let line = if state.detail.modal_open() || state.error().is_some() {
        Line::default()
    } else if state.operation_pending() {
        widgets::loading("sending…  Esc: back · q: quit")
    } else if let Some(search) = active_search(state, pr_data, area.width) {
        search
    } else {
        widgets::footer(area.width, &footer_actions(state, tab), state.refreshing)
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// Review, merge, ask-again and decline hints: they act on the whole PR, so they are
/// shown wherever the keys work (Overview and Description). Each is what its
/// row in `bindings` says: lit, dimmed with the reason, or left out.
fn pr_action_hints(state: &DetailView<'_>) -> Vec<Hint> {
    ['a', 'v', 'm', 'p', 'x']
        .into_iter()
        .filter_map(|key| bindings::hint(state, key))
        .collect()
}

fn footer_actions(state: &DetailView<'_>, tab: DetailTab) -> Vec<Hint> {
    if state.detail.editor.has_draft() {
        return widgets::hints_on("c: resume draft");
    }
    // While a batched review is open, surface its state and finish/discard keys
    // on every tab — line comments queue into it from the Diff too.
    if state.store.capabilities.reviews() && state.pending_review().is_some() {
        let mut parts: Vec<Hint> = bindings::hint(state, 'v').into_iter().collect();
        if state.surface().diff_viewer().is_some_and(|viewer| {
            viewer.focused_pending().is_some() && viewer.focus == DiffFocus::Pane
        }) {
            parts.extend(bindings::hint(state, 'd'));
        }
        let target = state.comment_target();
        match target {
            Some(CommentTarget::Reply(_)) => parts.push(Hint::on("r: reply")),
            Some(_) => parts.push(Hint::on("c: comment")),
            None => {}
        }
        if state.reply_target().is_some() && !matches!(target, Some(CommentTarget::Reply(_))) {
            parts.push(Hint::on("r: reply"));
        }
        parts.extend(bindings::hint(state, 'V'));
        return parts;
    }
    // Overview is the conversation tab and home of the PR-level actions: `c`
    // posts a PR comment, `r` replies, `a`/`v` review, `m` merges, `x` declines.
    // Lifecycle actions stay visible but dimmed-with-reason when unavailable.
    if tab == DetailTab::Overview {
        let mut parts = vec![];
        parts.extend(bindings::hint(state, 'c'));
        parts.extend(bindings::hint(state, 'r'));
        if state.editable_selected().is_some() {
            parts.extend(bindings::hint(state, 'e'));
            parts.extend(bindings::hint(state, 'd'));
        }
        if state
            .focused_thread()
            .is_some_and(|thread| thread.handle.is_some())
        {
            parts.extend(bindings::hint(state, 'R'));
        }
        parts.extend(bindings::hint(state, 'f'));
        parts.extend(bindings::hint(state, 'i'));
        parts.extend(pr_action_hints(state));
        // Last, so that it is the first to go where the line is short.
        let unresolved = state.detail.overview.timeline.unresolved.len();
        if unresolved > 0 {
            parts.push(Hint::on(format!("u: unresolved ({unresolved})")));
        }
        return parts;
    }
    if let Some(view) = state.surface().diff_viewer() {
        if view.focus == DiffFocus::Tree {
            let rows = crate::tui::ui::components::diff_viewer::file_tree::build_visible_rows(
                state.diff_files(),
                &view.collapsed,
                &view.tree_search.query,
            );
            let mut hints = Vec::new();
            if let Some(row) = rows.get(view.cursor) {
                hints.push(Hint::on(match row {
                    crate::tui::ui::components::diff_viewer::file_tree::TreeRow::Dir { .. } => {
                        "enter: toggle folder"
                    }
                    crate::tui::ui::components::diff_viewer::file_tree::TreeRow::File {
                        ..
                    } => "enter: code",
                }));
            }
            hints.push(Hint::on("/: files"));
            hints.push(Hint::on("h/l: tabs"));
            return hints;
        }
        let mut hints = vec![Hint::on("esc: files")];
        if let Some(key) = view.focused_fold() {
            hints.insert(
                0,
                Hint::on(if view.opened_comments.contains(&key) {
                    "space: fold comment"
                } else {
                    "space: expand comment"
                }),
            );
        } else if let Some(thread) = view.focused_thread().filter(|thread| thread.resolved)
            && let Some(id) = thread.comment_id
        {
            hints.insert(
                0,
                Hint::on(if view.expanded_threads.contains(&id) {
                    "space: collapse thread"
                } else {
                    "space: expand thread"
                }),
            );
        }

        match state.comment_target() {
            Some(CommentTarget::Reply(_)) => hints.push(Hint::on("r: reply")),
            Some(_) => hints.push(Hint::on("c: comment")),
            None => {}
        }
        if state
            .focused_thread()
            .is_some_and(|thread| thread.handle.is_some())
        {
            hints.extend(bindings::hint(state, 'R'));
        }
        hints.push(Hint::on("/: search"));
        hints.push(Hint::on("h/l: tabs"));
        return hints;
    }
    match tab {
        DetailTab::Description => {
            let mut hints = widgets::hints_on(if state.detail.description.max_horizontal > 0 {
                "H/L: pan  j/k: scroll  h/l: tabs"
            } else {
                "h/l: tabs  j/k: scroll"
            });
            hints.extend(pr_action_hints(state));
            hints
        }
        DetailTab::Commits => widgets::hints_on("enter: open  /: search"),
        DetailTab::Builds if matches!(state.surface(), Surface::BuildLog) => {
            widgets::hints_on("j/k: scroll  n/N: error  esc: back")
        }
        DetailTab::Builds => {
            let input = BuildsInput::new(state.pr_id, state.data);
            let mut hints = widgets::hints_on("j/k: move  h/l: tabs");
            if state.detail.builds.log_under_cursor(&input) {
                hints.extend(widgets::hints_on("enter: log"));
            }
            hints.extend(bindings::hint(state, 'b'));
            hints
        }
        // Both returned above with hints of their own.
        DetailTab::Overview | DetailTab::Diff => Vec::new(),
    }
}

fn active_search(
    state: &DetailView<'_>,
    pr_data: Option<&PrData>,
    width: u16,
) -> Option<Line<'static>> {
    let view = match state.surface() {
        Surface::CommitList => {
            return commits_search_prompt(&state.detail.commits.search, pr_data, width);
        }
        Surface::Diff(view) | Surface::CommitDiff(view) => view,
        Surface::Description | Surface::Overview | Surface::Builds | Surface::BuildLog => {
            return None;
        }
    };
    match view.focus {
        DiffFocus::Tree => tree_search_prompt(&view.tree_search, state.diff_files(), width),
        DiffFocus::Pane => pane_search_prompt(&view.pane_search, view.pane.matches.len(), width),
    }
}

fn commits_search_prompt(
    search: &SearchInput,
    pr_data: Option<&PrData>,
    width: u16,
) -> Option<Line<'static>> {
    search.open.then(|| {
        let count = match pr_data.map(|d| &d.commits) {
            Some(LoadState::Loaded(commits)) => search.filter_commits(commits).len(),
            _ => 0,
        };
        widgets::search_prompt(&search.query, count, width)
    })
}

fn tree_search_prompt(
    search: &SearchInput,
    files: &[FileDiff],
    width: u16,
) -> Option<Line<'static>> {
    search.open.then(|| {
        let count = files.iter().filter(|f| search.matches(&f.path)).count();
        widgets::search_prompt(&search.query, count, width)
    })
}

fn pane_search_prompt(
    search: &SearchInput,
    match_count: usize,
    width: u16,
) -> Option<Line<'static>> {
    if search.open {
        return Some(Line::from(widgets::search_input_spans(&search.query)));
    }
    if search.query.is_empty() {
        return None;
    }
    let theme = theme::current();
    let label = if match_count == 1 {
        "1 match".to_string()
    } else {
        format!("{match_count} matches")
    };
    let left = vec![Span::styled(
        format!("  /{}", search.query),
        Style::default().fg(theme.muted),
    )];
    let right = vec![Span::styled(
        format!("{label}   n/N: navigate   esc: clear  "),
        Style::default().fg(theme.muted),
    )];
    Some(Line::from(widgets::justify_between(
        left,
        right,
        width as usize,
    )))
}
