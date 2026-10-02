use crate::{
    app::{
        reviews::CommentTarget,
        store::{LoadState, PrData},
    },
    domain::{capabilities::Feature, diff::FileDiff},
    tui::{
        components::{diff_viewer::DiffFocus, search_input::SearchInput},
        screens::pr_detail::{DetailView, Surface, tabs::DetailTab},
        theme,
        widgets::{self, Hint},
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

/// Review, merge and decline hints: they act on the whole PR, so they are
/// shown wherever the keys work (Overview and Description).
fn pr_action_hints(state: &DetailView<'_>) -> Vec<Hint> {
    let mut parts = vec![];
    if state.store.capabilities.reviews() {
        parts.push(Hint::on("a: submit review"));
        parts.push(Hint::on("v: start review"));
    }
    if !state.store.capabilities.merge_strategies.is_empty() {
        parts.push(match state.merge_blocked_reason() {
            None => Hint::on("m: merge"),
            Some(reason) => Hint::off(format!("m: merge ({reason})")),
        });
    }
    if state.pr_is_declined() && state.store.capabilities.supports(Feature::ReopenPr) {
        parts.push(Hint::on("x: reopen"));
    } else if state.store.capabilities.supports(Feature::ClosePr) {
        parts.push(match state.decline_blocked_reason() {
            None => Hint::on("x: decline"),
            Some(reason) => Hint::off(format!("x: decline ({reason})")),
        });
    }
    parts
}

fn footer_actions(state: &DetailView<'_>, tab: DetailTab) -> Vec<Hint> {
    if state.detail.editor.has_draft() {
        return widgets::hints_on("c: resume draft");
    }
    // While a batched review is open, surface its state and finish/discard keys
    // on every tab — line comments queue into it from the Diff too.
    if state.store.capabilities.reviews()
        && let Some(review) = state.pending_review()
    {
        let mut parts = vec![Hint::on(format!(
            "v: finish draft ({})",
            review.comments.len()
        ))];
        if state.surface().diff_viewer().is_some_and(|viewer| {
            viewer.focused_pending().is_some() && viewer.focus == DiffFocus::Pane
        }) {
            parts.push(Hint::on("d: remove pending"));
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
        parts.push(Hint::on("V: discard"));
        return parts;
    }
    // Overview is the conversation tab and home of the PR-level actions: `c`
    // posts a PR comment, `r` replies, `a`/`v` review, `m` merges, `x` declines.
    // Lifecycle actions stay visible but dimmed-with-reason when unavailable.
    if tab == DetailTab::Overview {
        let mut parts = vec![];
        if state.comment_target().is_some() {
            parts.push(Hint::on("c: comment"));
        }
        if state.reply_target().is_some() {
            parts.push(Hint::on("r: reply"));
        }
        if state.editable_selected().is_some() {
            if state.store.capabilities.supports(Feature::EditComments) {
                parts.push(Hint::on("e: edit"));
            }
            if state.store.capabilities.supports(Feature::DeleteComments) {
                parts.push(Hint::on("d: delete"));
            }
        }
        if state.store.capabilities.supports(Feature::ResolveThreads)
            && let Some(thread) = state
                .focused_thread()
                .filter(|thread| thread.handle.is_some())
        {
            parts.push(Hint::on(if thread.resolved {
                "R: reopen thread"
            } else {
                "R: resolve thread"
            }));
        }
        if crate::tui::screens::pr_detail::tabs::overview::offers_filter(
            state.data,
            state.detail.overview.timeline.filter,
        ) {
            parts.push(Hint::on(format!(
                "f: comments ({})",
                state.detail.overview.timeline.filter.label()
            )));
        }
        parts.extend(pr_action_hints(state));
        return parts;
    }
    if let Some(view) = state.surface().diff_viewer() {
        if view.focus == DiffFocus::Tree {
            let rows = crate::tui::components::diff_viewer::file_tree::build_visible_rows(
                state.diff_files(),
                &view.collapsed,
                &view.tree_search.query,
            );
            let mut hints = Vec::new();
            if let Some(row) = rows.get(view.cursor) {
                hints.push(Hint::on(match row {
                    crate::tui::components::diff_viewer::file_tree::TreeRow::Dir { .. } => {
                        "enter: toggle folder"
                    }
                    crate::tui::components::diff_viewer::file_tree::TreeRow::File { .. } => {
                        "enter: code"
                    }
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
        if state.store.capabilities.supports(Feature::ResolveThreads)
            && let Some(thread) = state
                .focused_thread()
                .filter(|thread| thread.handle.is_some())
        {
            hints.push(Hint::on(if thread.resolved {
                "R: reopen thread"
            } else {
                "R: resolve thread"
            }));
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
        DetailTab::Builds => widgets::hints_on("j/k: scroll  h/l: tabs"),
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
        Surface::Description | Surface::Overview | Surface::Builds => return None,
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
