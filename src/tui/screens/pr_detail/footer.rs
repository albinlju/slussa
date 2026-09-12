use crate::{
    app::{
        navigation::Screen,
        reviews::CommentTarget,
        store::{LoadState, PrData},
    },
    domain::{capabilities::Feature, diff::FileDiff},
    tui::{
        components::{diff_viewer::DiffFocus, search_input::SearchInput},
        screens::pr_detail::{DetailView, tabs::DetailTab},
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

pub(super) fn render(
    frame: &mut Frame,
    state: &DetailView<'_>,
    pr_data: Option<&PrData>,
    tab: DetailTab,
    area: Rect,
) {
    let line = if state.detail.editor.is_open() {
        Line::default()
    } else if state.operation_pending() {
        widgets::loading("sending…  Esc: back · q: quit")
    } else if let Some(search) = active_search(state, pr_data, tab, area.width) {
        search
    } else {
        widgets::footer(area.width, &footer_actions(state, tab), state.refreshing)
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn footer_actions(state: &DetailView<'_>, tab: DetailTab) -> Vec<Hint> {
    if state.detail.editor.draft.is_some() {
        return widgets::hints_on("c: resume draft");
    }
    // While a batched review is open, surface its state and finish/discard keys
    // on every tab — line comments queue into it from the Diff too.
    if state.store.capabilities.reviews()
        && let Some(review) = state.pending_review()
    {
        let mut parts = vec![Hint::on(format!("reviewing ({})", review.comments.len()))];
        if state.comment_target().is_some() {
            parts.push(Hint::on("c: comment"));
        }
        if state.reply_target().is_some() {
            parts.push(Hint::on("r: reply"));
        }
        parts.push(Hint::on("v: finish"));
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
        if state.store.capabilities.reviews() {
            parts.push(Hint::on("a: verdict"));
            parts.push(Hint::on("v: review"));
        }
        if let Screen::Detail { pr_id, .. } = state.screen {
            if !state.store.capabilities.merge_strategies.is_empty() {
                parts.push(match state.merge_blocked_reason(pr_id) {
                    None => Hint::on("m: merge"),
                    Some(reason) => Hint::off(format!("m: merge ({reason})")),
                });
            }
            if state.store.capabilities.supports(Feature::ClosePr) {
                parts.push(match state.decline_blocked_reason(pr_id) {
                    None => Hint::on("x: decline"),
                    Some(reason) => Hint::off(format!("x: decline ({reason})")),
                });
            }
        }
        return parts;
    }
    // Other tabs: only the line-comment hint, shown once you're on a row in the
    // diff pane (a thread turns it into a reply).
    match state.comment_target() {
        Some(CommentTarget::Reply(_)) => vec![Hint::on("c: reply")],
        Some(_) => vec![Hint::on("c: comment")],
        None => match tab {
            DetailTab::Description => widgets::hints_on("h/l: tabs  j/k: scroll"),
            DetailTab::Diff => widgets::hints_on("enter: code  /: files"),
            DetailTab::Commits if state.detail.commits.open_commit.is_none() => {
                widgets::hints_on("enter: open  /: search")
            }
            DetailTab::Builds => widgets::hints_on("h/l: tabs"),
            _ => Vec::new(),
        },
    }
}

fn active_search(
    state: &DetailView<'_>,
    pr_data: Option<&PrData>,
    tab: DetailTab,
    width: u16,
) -> Option<Line<'static>> {
    let viewing_commit = state.detail.commits.open_commit.is_some();
    if tab == DetailTab::Commits && !viewing_commit {
        return commits_search_prompt(&state.detail.commits.search, pr_data, width);
    }
    if tab != DetailTab::Diff && !(tab == DetailTab::Commits && viewing_commit) {
        return None;
    }
    let view = state.detail.active_diff_view();
    match view.focus {
        DiffFocus::Tree => tree_search_prompt(&view.tree_search, tree_files(state, pr_data), width),
        DiffFocus::Pane => pane_search_prompt(&view.pane_search, view.pane_matches.len(), width),
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

fn tree_files<'a>(state: &DetailView<'_>, pr_data: Option<&'a PrData>) -> &'a [FileDiff] {
    match pr_data.and_then(|d| d.diff_for(state.detail.commits.open_commit.as_deref())) {
        Some(LoadState::Loaded(diff)) => &diff.files,
        _ => &[],
    }
}
