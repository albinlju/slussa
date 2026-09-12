use crate::{
    app::{
        action::{Action, CommitsAction, DetailAction, DiffAction, SearchAction},
        navigation::Screen,
    },
    tui::{
        component::Component,
        components::diff_viewer::{DiffContext, DiffFocus},
        screens::pr_detail::tabs::DetailTab,
    },
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(in crate::tui) fn key_to_action(
    state: &super::DetailView<'_>,
    key: KeyEvent,
) -> Option<Action> {
    // An error popup is modal: any key dismisses it.
    if state.error().is_some() {
        return Some(Action::Detail(DetailAction::DismissError));
    }
    if state.operation_pending() && state.detail.editor.is_open() {
        return match key.code {
            KeyCode::Esc => Some(Action::Detail(DetailAction::Back)),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }
    if state.detail.editor.is_open() {
        return state.detail.editor.handle_key(key, &false);
    }
    if key.code == KeyCode::Char('q') {
        return Some(Action::Quit);
    }
    let Screen::Detail { tab, pr_id } = state.screen else {
        return None;
    };
    let viewing_commit = state.detail.commits.open_commit.is_some();
    let code = key.code;
    // Single-letter actions fire only unmodified, so Ctrl-d/Ctrl-e/etc. (scroll,
    // muscle memory) don't accidentally trigger comment/approve actions.
    let plain = key.modifiers.is_empty();

    if let Some(dialog) = &state.detail.confirm {
        return dialog.handle_key(key, &());
    }

    if let Some(dialog) = &state.detail.review_picker {
        return dialog.handle_key(key, &state.review_context());
    }

    if let Some(dialog) = &state.detail.merge_picker {
        return dialog.handle_key(key, &state.store.capabilities.merge_strategies.as_slice());
    }

    if code == KeyCode::Char('?') {
        return Some(Action::Detail(DetailAction::ToggleHelp));
    }
    if state.detail.help_open {
        return if code == KeyCode::Esc {
            Some(Action::Detail(DetailAction::ToggleHelp))
        } else {
            state.detail.help.handle_key(key, &&[][..])
        };
    }

    if plain && state.has_pr_link() {
        let kind = match code {
            KeyCode::Char('o') => Some(crate::app::action::LinkAction::Open),
            KeyCode::Char('y') => Some(crate::app::action::LinkAction::Copy),
            _ => None,
        };
        if let Some(kind) = kind {
            return Some(Action::PrLink { pr_id, kind });
        }
    }
    // `a` opens the review-verdict menu. Always available — even on your own PR
    // you can leave a comment review; the picker dims the verdicts you can't use.
    if plain && code == KeyCode::Char('a') && tab == DetailTab::Overview {
        return Some(Action::Detail(DetailAction::OpenReviewPicker));
    }
    // `v` runs the batched review: it starts a review the first time, then finishes
    // it (opening the verdict menu) once one's in progress. Line comments made
    // while it's open queue into the review instead of posting.
    if plain
        && code == KeyCode::Char('v')
        && (state.pending_review().is_some()
            || tab == DetailTab::Overview
            || tab == DetailTab::Diff)
    {
        return Some(Action::Detail(if state.pending_review().is_some() {
            DetailAction::FinishReview
        } else {
            DetailAction::StartReview
        }));
    }
    // Shift+V discards an in-progress review and its queued comments.
    if code == KeyCode::Char('V') && state.pending_review().is_some() {
        return Some(Action::Detail(DetailAction::AbandonReview));
    }
    // `m` opens the merge-strategy menu — only when the PR is mergeable. You can
    // merge your own PR, so (unlike `a`) there's no own-PR gate.
    if plain && code == KeyCode::Char('m') && tab == DetailTab::Overview && state.can_merge(pr_id) {
        return Some(Action::Detail(DetailAction::OpenMergePicker));
    }
    // `x` declines/closes the PR (with a confirm) while it's still open.
    if plain && code == KeyCode::Char('x') && tab == DetailTab::Overview && state.pr_is_open(pr_id)
    {
        return Some(Action::Detail(DetailAction::OpenDecline));
    }
    if plain && code == KeyCode::Char('c') {
        return Some(Action::Detail(DetailAction::OpenComment));
    }
    if plain && code == KeyCode::Char('r') {
        return Some(Action::Detail(DetailAction::OpenReply));
    }
    // Shift+R toggles resolve on the focused thread (application no-ops off-thread).
    if code == KeyCode::Char('R') {
        return Some(Action::Detail(DetailAction::ResolveThread));
    }
    if code == KeyCode::Char('F') {
        return Some(Action::Refresh);
    }
    // Overview-only: step individual comments within the focused block (Ctrl-j/k),
    // then edit/delete the one you land on (the application gates on authorship).
    if tab == DetailTab::Overview {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let crate::app::store::LoadState::Loaded(prs) = &state.store.cache.prs else {
                return None;
            };
            if let Some(pr) = prs.iter().find(|p| p.id == pr_id)
                && let Some(action) = state.detail.overview.handle_key(
                    key,
                    &crate::tui::screens::pr_detail::tabs::overview::OverviewContext {
                        pr,
                        data: state.store.cache.details.get(&pr_id),
                        capabilities: &state.store.capabilities,
                    },
                )
            {
                return Some(action);
            }
        }
        if plain && code == KeyCode::Char('e') {
            return Some(Action::Detail(DetailAction::EditComment));
        }
        if plain && code == KeyCode::Char('d') {
            return Some(Action::Detail(DetailAction::DeleteComment));
        }
    }

    // `d` on a queued review comment (diff pane) removes it from the review.
    if plain
        && code == KeyCode::Char('d')
        && (tab == DetailTab::Diff || (tab == DetailTab::Commits && viewing_commit))
        && state.detail.active_diff_view().pane_pending.is_some()
    {
        return Some(Action::Detail(DetailAction::RemovePendingComment));
    }

    escape_action(state, tab, viewing_commit, code)
        .or_else(|| tab_select_key(code))
        .or_else(|| tab_key(state, tab, viewing_commit, code))
        // `[`/`]` switch tabs, but only after tab_key so the commit-diff view
        // keeps them for stepping commits.
        .or_else(|| tab_bracket_key(code))
}

fn escape_action(
    state: &super::DetailView<'_>,
    tab: DetailTab,
    viewing_commit: bool,
    code: KeyCode,
) -> Option<Action> {
    if code != KeyCode::Esc {
        return None;
    }
    let view = state.detail.active_diff_view();
    let in_diff_pane = view.focus == DiffFocus::Pane
        && (tab == DetailTab::Diff || (tab == DetailTab::Commits && viewing_commit));
    if in_diff_pane {
        if !view.pane_search.query.is_empty() {
            return Some(Action::Search(SearchAction::Cancel));
        }
        return Some(Action::Diff(DiffAction::FocusTree));
    }
    if tab == DetailTab::Commits && viewing_commit {
        return Some(Action::Commits(CommitsAction::Back));
    }
    Some(Action::Detail(DetailAction::Back))
}

fn tab_select_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            DetailTab::ALL
                .get(idx)
                .map(|&t| Action::Detail(DetailAction::SelectTab(t)))
        }
        _ => None,
    }
}

fn tab_bracket_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('[') => Some(Action::Detail(DetailAction::PrevTab)),
        KeyCode::Char(']') => Some(Action::Detail(DetailAction::NextTab)),
        _ => None,
    }
}

fn tab_key(
    state: &super::DetailView<'_>,
    tab: DetailTab,
    viewing_commit: bool,
    code: KeyCode,
) -> Option<Action> {
    match tab {
        DetailTab::Diff => state.detail.active_diff_view().handle_key(
            KeyEvent::new(code, KeyModifiers::NONE),
            &DiffContext {
                diff: None,
                threads: &[],
                pending: &[],
                author: "",
            },
        ),
        DetailTab::Commits if viewing_commit => match code {
            KeyCode::Char('[') => Some(Action::Commits(CommitsAction::StepCommit(-1))),
            KeyCode::Char(']') => Some(Action::Commits(CommitsAction::StepCommit(1))),
            _ => state.detail.active_diff_view().handle_key(
                KeyEvent::new(code, KeyModifiers::NONE),
                &DiffContext {
                    diff: None,
                    threads: &[],
                    pending: &[],
                    author: "",
                },
            ),
        },
        DetailTab::Commits => {
            let Screen::Detail { pr_id, .. } = state.screen else {
                return None;
            };
            state
                .detail
                .commits
                .handle_key(
                    KeyEvent::new(code, KeyModifiers::NONE),
                    &super::tabs::commits::CommitContext {
                        pr_id,
                        data: state.store.cache.details.get(&pr_id),
                        pending: &[],
                        author: "",
                    },
                )
                .or_else(|| tab_nav(code))
        }
        DetailTab::Overview | DetailTab::Description => {
            let Screen::Detail { pr_id, .. } = state.screen else {
                return None;
            };
            let crate::app::store::LoadState::Loaded(prs) = &state.store.cache.prs else {
                return tab_nav(code);
            };
            let Some(pr) = prs.iter().find(|pr| pr.id == pr_id) else {
                return tab_nav(code);
            };
            let key = KeyEvent::new(code, KeyModifiers::NONE);
            let action = if tab == DetailTab::Overview {
                state.detail.overview.handle_key(
                    key,
                    &crate::tui::screens::pr_detail::tabs::overview::OverviewContext {
                        pr,
                        data: state.store.cache.details.get(&pr_id),
                        capabilities: &state.store.capabilities,
                    },
                )
            } else {
                state.detail.description.handle_key(key, &pr)
            };
            action.or_else(|| tab_nav(code))
        }
        DetailTab::Builds => tab_nav(code),
    }
}

fn tab_nav(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
        _ => None,
    }
}
