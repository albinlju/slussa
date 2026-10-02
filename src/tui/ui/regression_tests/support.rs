//! Imports and helpers shared by the files in this directory.

pub(super) use crate::tui::ui::*;
pub(super) use crate::{
    domain::{
        activity::Activity,
        authorship::Authorship,
        ci::CiSummary,
        comment::{Comment, CommentId, CommentThread, ThreadAnchor, ThreadHandle},
        commit::{Commit, CommitOid},
        diff::*,
        pr::*,
        review::CommentTarget,
        user::{AccountKind, User},
    },
    tui::{
        app::{
            navigation::Screen,
            state::*,
            store::{LoadState, PrData},
        },
        ui::{
            action::*,
            components::{
                comment_editor::{CommentDraft, CommentEditor, EditorView},
                diff_viewer::{DiffFocus, FocusedNav, NavTarget, PaneNav},
                help_dialog::HelpDialog,
            },
            screens::pr_detail::{
                Overlay,
                dialogs::{
                    PrSummary,
                    confirm::{ConfirmDialog, ConfirmKind},
                    merge::MergeDialog,
                    review::ReviewDialog,
                },
                tabs::DetailTab,
            },
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
        id: PrId(42),
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
        PrId(42),
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
            mergeability: LoadState::Loaded(Mergeability::Mergeable),
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
        if let Some(Effect::Navigate(screen)) = effect {
            state.screen = screen;
        }
    }
}

/// A comment by a person, made now, with no thread of its own.
pub(super) fn comment(id: u64, content: &str) -> Comment {
    Comment {
        id: Some(CommentId(id)),
        author: User {
            username: "alice".into(),
        },
        account: AccountKind::Person,
        authorship: Authorship::Human,
        content: content.into(),
        created: chrono::Utc::now(),
        reactions: vec![],
        reply_to: None,
    }
}

/// A comment by a bot account.
pub(super) fn bot_comment(id: u64, content: &str) -> Comment {
    Comment {
        account: AccountKind::Bot,
        authorship: Authorship::Ai,
        ..comment(id, content)
    }
}

/// A review thread on the fixture's file, on line 1, whose root comment is `root`.
pub(super) fn thread(root: u64, resolved: bool, comments: Vec<Comment>) -> CommentThread {
    CommentThread {
        comments,
        reply_to: Some(CommentId(root)),
        anchor: Some(ThreadAnchor {
            revision: None,
            path: "src/main.rs".into(),
            line: Some(LineRef::New(1)),
            resolved,
            handle: Some(ThreadHandle::NodeId("thread".into())),
        }),
    }
}

/// The fixture's PR #42 open on `tab`, with this activity loaded.
pub(super) fn pr_on(tab: DetailTab, activity: Activity) -> AppState {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab,
    };
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(activity);
    state
}

/// The Overview with these comments by people, in the order given.
pub(super) fn overview_with(contents: &[&str]) -> AppState {
    let comments = (1..)
        .zip(contents)
        .map(|(id, content)| comment(id, content))
        .collect();
    pr_on(
        DetailTab::Overview,
        Activity {
            comments,
            ..Activity::default()
        },
    )
}

/// The screen drawn at `width` by `height`, as the text of its cells with no line
/// breaks.
pub(super) fn draw(state: &mut AppState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    rendered_text(&terminal)
}

/// The screen at the usual 140 by 30.
pub(super) fn screen(state: &mut AppState) -> String {
    draw(state, 140, 30)
}

/// The last row of a screen drawn 140 wide: the footer.
pub(super) fn footer_of(text: &str) -> String {
    text.chars()
        .rev()
        .take(140)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
