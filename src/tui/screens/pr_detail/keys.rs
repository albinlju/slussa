use crate::{
    app::action::{
        Action, CommitsAction, DetailAction, DiffAction, Effect, NavAction, PrAction, SearchAction,
    },
    tui::{
        component::Component,
        components::diff_viewer::DiffFocus,
        screens::pr_detail::{Overlay, Surface, tabs::DetailTab},
    },
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(in crate::tui) fn key_to_action(
    state: &super::DetailView<'_>,
    key: KeyEvent,
) -> Option<Action> {
    // Errors capture input; navigation reads the message rather than acting on the PR.
    if state.error().is_some() {
        return state.detail.error.handle_key(key, &());
    }
    if state.operation_pending() && state.detail.editor.is_open() {
        return match key.code {
            KeyCode::Esc => Some(Action::from(NavAction::Back)),
            KeyCode::Char('q') => Some(Action::Effect(Effect::Quit)),
            _ => None,
        };
    }
    if state.detail.editor.is_open() {
        return state.detail.editor.handle_key(key, &());
    }
    if key.code == KeyCode::Char('q') {
        return Some(Action::Effect(Effect::Quit));
    }
    let (tab, pr_id) = (state.tab, state.pr_id);
    let surface = state.surface();
    let code = key.code;
    // Single-letter actions fire only unmodified, so Ctrl-d/Ctrl-e/etc. (scroll,
    // muscle memory) don't accidentally trigger comment/approve actions.
    let plain = key.modifiers.is_empty();

    match &state.detail.overlay {
        Some(Overlay::Confirm(dialog)) => return dialog.handle_key(key, &()),
        Some(Overlay::Review(dialog)) => return dialog.handle_key(key, &state.review_context()),
        Some(Overlay::Merge(dialog)) => {
            return dialog.handle_key(key, &state.store.capabilities.merge_strategies.as_slice());
        }
        Some(Overlay::Help(help)) => {
            return if matches!(code, KeyCode::Esc | KeyCode::Char('?')) {
                Some(Action::from(NavAction::ToggleHelp))
            } else {
                help.handle_key(key, &())
            };
        }
        None => {}
    }
    if code == KeyCode::Char('?') {
        return Some(Action::from(NavAction::ToggleHelp));
    }

    if plain && state.has_pr_link() {
        let kind = match code {
            KeyCode::Char('o') => Some(crate::app::action::LinkAction::Open),
            KeyCode::Char('y') => Some(crate::app::action::LinkAction::Copy),
            _ => None,
        };
        if let Some(kind) = kind {
            return Some(Action::Effect(Effect::PrLink { pr_id, kind }));
        }
    }
    // The PR-level actions (`a`, `v`, `m`, `x`) work on the tabs that read the PR
    // itself, so a PR can be acted on straight from its description.
    let acts_on_pr = matches!(tab, DetailTab::Overview | DetailTab::Description);
    // `a` opens the review-verdict menu. Always available — even on your own PR
    // you can leave a comment review; the picker dims the verdicts you can't use.
    if plain && code == KeyCode::Char('a') && acts_on_pr {
        return Some(Action::from(PrAction::OpenReviewPicker));
    }
    // `v` runs the batched review: it starts a review the first time, then finishes
    // it (opening the verdict menu) once one's in progress. Line comments made
    // while it's open queue into the review instead of posting.
    if plain
        && code == KeyCode::Char('v')
        && (state.pending_review().is_some() || acts_on_pr || tab == DetailTab::Diff)
    {
        return Some(Action::from(if state.pending_review().is_some() {
            PrAction::FinishReview
        } else {
            PrAction::StartReview
        }));
    }
    // Shift+V discards an in-progress review and its queued comments.
    if code == KeyCode::Char('V') && state.pending_review().is_some() {
        return Some(Action::from(PrAction::AbandonReview));
    }
    // `m` opens the merge-strategy menu — only when the PR is mergeable. You can
    // merge your own PR, so (unlike `a`) there's no own-PR gate.
    if plain && code == KeyCode::Char('m') && acts_on_pr && state.can_merge() {
        return Some(Action::from(PrAction::OpenMergePicker));
    }
    // `x` declines/closes the PR (with a confirm) while it's still open, and
    // reopens it (with a confirm) once it has been declined.
    if plain && code == KeyCode::Char('x') && acts_on_pr {
        if state.pr_is_open() {
            return Some(Action::from(PrAction::OpenDecline));
        }
        if state.pr_is_declined() && state.supports_action(DetailAction::Pr(PrAction::OpenReopen)) {
            return Some(Action::from(PrAction::OpenReopen));
        }
    }
    if plain && code == KeyCode::Char('c') {
        return Some(Action::from(PrAction::OpenComment));
    }
    if plain && code == KeyCode::Char('r') {
        return Some(Action::from(PrAction::OpenReply));
    }
    // Shift+R toggles resolve on the focused thread (application no-ops off-thread).
    if code == KeyCode::Char('R') {
        return Some(Action::from(PrAction::ResolveThread));
    }
    if code == KeyCode::Char('F') {
        return Some(Action::Effect(Effect::Refresh));
    }
    // Overview-only: step individual comments within the focused block (Ctrl-j/k),
    // then edit/delete the one you land on (the application gates on authorship).
    if tab == DetailTab::Overview {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && let Some(action) = state.detail.overview.handle_key(key, &())
        {
            return Some(action);
        }
        if plain && code == KeyCode::Char('e') {
            return Some(Action::from(PrAction::EditComment));
        }
        if plain && code == KeyCode::Char('d') {
            return Some(Action::from(PrAction::DeleteComment));
        }
    }

    // `d` on a queued review comment (diff pane) removes it from the review.
    if plain
        && code == KeyCode::Char('d')
        && surface
            .diff_viewer()
            .is_some_and(|viewer| viewer.pane_pending.is_some())
    {
        return Some(Action::Detail(DetailAction::Pr(
            PrAction::RemovePendingComment,
        )));
    }

    escape_action(surface, code)
        .or_else(|| tab_select_key(code))
        .or_else(|| tab_key(state, surface, code))
        // `[`/`]` switch tabs, but only after tab_key so the commit-diff view
        // keeps them for stepping commits.
        .or_else(|| tab_bracket_key(code))
}

/// Esc steps out one level: the pane's search, the pane, the open commit, and
/// then the PR.
fn escape_action(surface: Surface<'_>, code: KeyCode) -> Option<Action> {
    if code != KeyCode::Esc {
        return None;
    }
    if let Some(view) = surface.diff_viewer()
        && view.focus == DiffFocus::Pane
    {
        if !view.pane_search.query.is_empty() {
            return Some(Action::Search(SearchAction::Cancel));
        }
        return Some(Action::Diff(DiffAction::FocusTree));
    }
    Some(match surface {
        Surface::CommitDiff(_) => Action::Commits(CommitsAction::Back),
        Surface::Description
        | Surface::Overview
        | Surface::Diff(_)
        | Surface::CommitList
        | Surface::Builds => Action::from(NavAction::Back),
    })
}

fn tab_select_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            DetailTab::ALL
                .get(idx)
                .map(|&t| Action::Detail(DetailAction::Nav(NavAction::SelectTab(t))))
        }
        _ => None,
    }
}

fn tab_bracket_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('[') => Some(Action::from(NavAction::PrevTab)),
        KeyCode::Char(']') => Some(Action::from(NavAction::NextTab)),
        _ => None,
    }
}

fn tab_key(state: &super::DetailView<'_>, surface: Surface<'_>, code: KeyCode) -> Option<Action> {
    match surface {
        Surface::Diff(viewer) => viewer
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &state.diff_files())
            .or_else(|| tab_letters(code)),
        Surface::CommitDiff(viewer) => match code {
            KeyCode::Char('[') => Some(Action::Commits(CommitsAction::StepCommit(-1))),
            KeyCode::Char(']') => Some(Action::Commits(CommitsAction::StepCommit(1))),
            _ => viewer
                .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &state.diff_files())
                .or_else(|| tab_letters(code)),
        },
        Surface::CommitList => state
            .detail
            .commits
            .handle_key(
                KeyEvent::new(code, KeyModifiers::NONE),
                &super::tabs::commits::CommitInput::new(state.pr_id, state.data),
            )
            .or_else(|| tab_nav(code)),
        Surface::Overview => state
            .detail
            .overview
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &())
            .or_else(|| tab_nav(code)),
        Surface::Description => state
            .detail
            .description
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &())
            .or_else(|| tab_nav(code)),
        Surface::Builds => state
            .detail
            .builds
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &())
            .or_else(|| tab_nav(code)),
    }
}

/// `h` and `l` change tab on every tab. The arrow keys do too, except in the
/// diff, where they move between the file tree and the code.
fn tab_letters(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('l') => Some(Action::from(NavAction::NextTab)),
        KeyCode::Char('h') => Some(Action::from(NavAction::PrevTab)),
        _ => None,
    }
}

fn tab_nav(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Right | KeyCode::Char('l') => Some(Action::from(NavAction::NextTab)),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::from(NavAction::PrevTab)),
        _ => None,
    }
}
