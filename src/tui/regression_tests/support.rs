//! Imports and helpers shared by the files in this directory.

pub(super) use crate::tui::*;
pub(super) use crate::{
    app::{
        action::{CommitsAction, DetailAction, DiffAction, ListAction},
        navigation::Screen,
        reviews::CommentTarget,
        state::*,
        store::{LoadState, PrData},
    },
    domain::{activity::Activity, ci::CiSummary, commit::Commit, diff::*, pr::*, user::User},
    tui::{
        components::{comment_editor::CommentDraft, diff_viewer::DiffFocus},
        screens::pr_detail::{
            dialogs::{
                confirm::{ConfirmDialog, ConfirmKind},
                merge::MergeDialog,
                review::ReviewDialog,
            },
            tabs::DetailTab,
        },
    },
};
pub(super) use ratatui::{Terminal, backend::TestBackend};
pub(super) use std::fmt::Write;

pub(crate) fn fixture() -> AppState {
    let now = chrono::Utc::now();
    let mut state = AppState::default();
    state.store.cache.prs = LoadState::Loaded(vec![PullRequest {
        url: Some("https://example.com/team/project/pull/42".into()),
        id: 42,
        title: "Component migration".into(),
        description: Some("Review **this change**.".into()),
        author: User {
            username: "alice".into(),
        },
        ci: CiSummary::Success,
        status: PrStatus::Open,
        reviewers: vec![],
        labels: vec!["rust".into()],
        comment_count: 0,
        source_branch: "feature".into(),
        target_branch: "main".into(),
        additions: 1,
        deletions: 1,
        changed_files: 1,
        created: now,
        updated: now,
    }]);
    state.store.cache.details.insert(
        42,
        PrData {
            diff: LoadState::Loaded(Diff {
                revision: None,
                files: vec![FileDiff {
                    path: "src/main.rs".into(),
                    hunks: vec![Hunk {
                        old_start: 1,
                        new_start: 1,
                        lines: vec![
                            DiffLine::Removed("old".into()),
                            DiffLine::Added("new".into()),
                        ],
                    }],
                }],
            }),
            commits: LoadState::Loaded(vec![Commit {
                oid: "abcdef123456".into(),
                headline: "Extract components".into(),
                author_name: "alice".into(),
                authored_at: now,
                additions: 1,
                deletions: 1,
            }]),
            activity: LoadState::Loaded(Activity {
                comments: vec![],
                events: vec![],
                threads: vec![],
            }),
            builds: LoadState::Loaded(vec![]),
            mergeability: LoadState::Loaded(MergeStatus::new(Mergeability::Mergeable)),
            info: LoadState::Loaded(PrInfo {
                description: Some("Review **this change**.".into()),
                labels: vec!["rust".into()],
            }),
            ..PrData::default()
        },
    );
    state.store.capabilities = crate::providers::Provider::GitHub.capabilities();
    state.store.capabilities.merge_strategies = vec![MergeStrategy::Merge, MergeStrategy::Squash];
    state
}

pub(super) fn key(state: &AppState, code: KeyCode) -> Action {
    key_to_action(state, KeyEvent::new(code, KeyModifiers::NONE)).unwrap()
}

pub(super) fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

pub(super) fn local_key(state: &mut AppState, code: KeyCode) {
    if let Some(action) = key_to_action(state, KeyEvent::new(code, KeyModifiers::NONE)) {
        let effect = state.ui.update(action, &state.store, state.screen);
        if let Some(Action::Navigate(screen)) = effect {
            state.screen = screen;
        }
    }
}
