use super::*;
use crate::{
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
use ratatui::{Terminal, backend::TestBackend};
use std::fmt::Write;

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
            mergeability: LoadState::Loaded(Mergeability::Mergeable),
            ..PrData::default()
        },
    );
    state.store.capabilities = crate::providers::Provider::GitHub.capabilities();
    state.store.capabilities.merge_strategies = vec![MergeStrategy::Merge, MergeStrategy::Squash];
    state
}

#[test]
fn screens_preserve_rendered_output() {
    let mut output = String::new();
    for (width, height) in [(100, 30), (40, 12)] {
        for screen in std::iter::once(Screen::List)
            .chain(DetailTab::ALL.map(|tab| Screen::Detail { pr_id: 42, tab }))
        {
            let mut state = fixture();
            state.screen = screen;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render(frame, &mut state)).unwrap();
            writeln!(output, "{width}x{height} {screen:?}").unwrap();
            let buffer = terminal.backend().buffer();
            for y in 0..height {
                let mut row = String::new();
                for x in 0..width {
                    row.push_str(buffer[(x, y)].symbol());
                }
                output.push_str(row.trim_end());
                output.push('\n');
            }
        }
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tui/testdata/screens.txt");
    if std::env::var_os("TUIPR_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(path, &output).unwrap();
    }
    assert_eq!(output, std::fs::read_to_string(path).unwrap());
}

fn key(state: &AppState, code: KeyCode) -> Action {
    key_to_action(state, KeyEvent::new(code, KeyModifiers::NONE)).unwrap()
}

#[test]
fn keyboard_routes_list_diff_commit_and_editor() {
    let mut state = fixture();
    assert!(matches!(
        key(&state, KeyCode::Enter),
        Action::List(ListAction::OpenPr(42))
    ));
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Diff,
    };
    assert!(matches!(
        key(&state, KeyCode::Char('j')),
        Action::Diff(DiffAction::MoveCursor(1))
    ));
    state.ui.detail.diff.focus = DiffFocus::Pane;
    assert!(matches!(
        key(&state, KeyCode::Esc),
        Action::Diff(DiffAction::FocusTree)
    ));
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Commits,
    };
    assert!(matches!(
        key(&state, KeyCode::Enter),
        Action::Commits(CommitsAction::Open)
    ));
    state.ui.detail.editor.draft = Some(CommentDraft {
        target: CommentTarget::Pr,
        text: String::new(),
    });
    assert!(matches!(
        key(&state, KeyCode::Char('q')),
        Action::Detail(DetailAction::CommentType('q'))
    ));
    assert!(matches!(
        key(&state, KeyCode::Enter),
        Action::Detail(DetailAction::CommentType('\n'))
    ));
}

#[test]
fn filter_and_confirmation_capture_navigation() {
    let mut state = fixture();
    state.ui.list.filter_picker_open = true;
    assert!(matches!(
        key(&state, KeyCode::Char('j')),
        Action::List(ListAction::FilterPickerNext)
    ));
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Overview,
    };
    state.ui.detail.confirm = Some(ConfirmDialog::new(ConfirmKind::Decline));
    assert!(matches!(
        key(&state, KeyCode::Enter),
        Action::Detail(DetailAction::SubmitConfirm)
    ));
    assert!(matches!(
        key(&state, KeyCode::Esc),
        Action::Detail(DetailAction::CloseConfirm)
    ));
}

#[test]
fn extracted_dialogs_render_and_keep_their_key_bindings() {
    for dialog in ["confirm", "review", "merge", "error", "help"] {
        let mut state = fixture();
        state.screen = Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview,
        };
        let (label, close) = match dialog {
            "confirm" => {
                state.ui.detail.confirm = Some(ConfirmDialog::new(ConfirmKind::Decline));
                ("Decline this PR?", DetailAction::CloseConfirm)
            }
            "review" => {
                state.ui.detail.review_picker = Some(ReviewDialog::default());
                ("Submit review", DetailAction::CloseReviewPicker)
            }
            "merge" => {
                state.ui.detail.merge_picker = Some(MergeDialog::default());
                ("Merge this PR", DetailAction::CloseMergePicker)
            }
            "error" => {
                state.store.errors.insert(42, "Request failed".into());
                ("Request failed", DetailAction::DismissError)
            }
            "help" => {
                state.ui.detail.help_open = true;
                ("Help", DetailAction::ToggleHelp)
            }
            _ => unreachable!(),
        };
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains(label), "{dialog} content missing");
        let actual = key(&state, KeyCode::Esc);
        // DetailAction deliberately has no equality requirement in the app API.
        assert_eq!(
            format!("{actual:?}"),
            format!("{:?}", Action::Detail(close))
        );
        if dialog == "review" {
            assert!(matches!(
                key(&state, KeyCode::Enter),
                Action::Detail(DetailAction::ReviewSelect)
            ));
        }
        if dialog == "merge" {
            assert!(matches!(
                key(&state, KeyCode::Enter),
                Action::Detail(DetailAction::MergeSelect)
            ));
        }
    }
}

#[test]
fn dialog_instances_own_selection_and_respect_available_choices() {
    use crate::{
        domain::{pr::MergeStrategy, review::ReviewVerdict},
        tui::{component::Component, screens::pr_detail::dialogs::review::ReviewContext},
    };
    let mut confirm = ConfirmDialog::new(ConfirmKind::Decline);
    let other = ConfirmDialog::new(ConfirmKind::Decline);
    confirm.update(DetailAction::ConfirmMove(1), &());
    assert_eq!(confirm.accepted(), None);
    assert_eq!(other.accepted(), Some(ConfirmKind::Decline));

    let ctx = ReviewContext {
        options: vec![
            (ReviewVerdict::Approve, Some("your PR")),
            (ReviewVerdict::Comment, None),
        ],
        pending: None,
    };
    let mut review = ReviewDialog::new(&ctx);
    assert_eq!(review.selected(&ctx), Some(ReviewVerdict::Comment));
    review.update(DetailAction::ReviewMove(-1), &ctx);
    assert_eq!(review.selected(&ctx), None);

    let strategies = [MergeStrategy::Merge, MergeStrategy::Rebase];
    let mut merge = MergeDialog::default();
    merge.update(DetailAction::MergeMove(99), &strategies.as_slice());
    assert_eq!(merge.selected(&strategies), Some(MergeStrategy::Rebase));
}

#[test]
fn timeline_keeps_thread_selection_and_sidebar_is_responsive() {
    use crate::domain::comment::{Comment, CommentThread, ThreadAnchor};
    let mut state = fixture();
    state.store.current_user = "alice".into();
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Overview,
    };
    let comments = (10..12)
        .map(|id| Comment {
            id: Some(id),
            author: User {
                username: "alice".into(),
            },
            content: format!("Comment {id}"),
            created: chrono::Utc::now(),
            reactions: vec![],
            reply_to: Some(10),
        })
        .collect();
    state.store.cache.details.get_mut(&42).unwrap().activity = LoadState::Loaded(Activity {
        comments: vec![],
        events: vec![],
        threads: vec![CommentThread {
            comments,
            reply_to: Some(10),
            anchor: Some(ThreadAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: Some(1),
                old_line: None,
                resolved: false,
                node_id: Some("thread-1".into()),
            }),
        }],
    });
    for width in [100, 40] {
        let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert_eq!(text.contains("Reviewers"), width == 100);
        assert_eq!(state.ui.detail.overview.timeline.reply, Some(10));
        assert_eq!(
            state
                .detail_view()
                .focused_thread()
                .unwrap()
                .node_id
                .as_deref(),
            Some("thread-1")
        );
    }
    assert_eq!(
        state.detail_view().editable_selected().unwrap().id,
        Some(10)
    );
    let action = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(
        state
            .ui
            .update(action, &state.store, state.screen)
            .is_none()
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert_eq!(
        state.detail_view().editable_selected().unwrap().id,
        Some(11)
    );
}

#[test]
fn unsupported_features_are_hidden_from_content_footer_and_help() {
    use crate::domain::capabilities::{Capabilities, Feature};
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Overview,
    };
    state.store.capabilities = Capabilities::default();
    for help_open in [false, true] {
        state.ui.detail.help_open = help_open;
        let mut terminal = Terminal::new(TestBackend::new(150, 50)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        for hidden in [
            "Builds",
            "mergeable",
            "c: comment",
            "a: verdict",
            "v: review",
            "m: merge",
            "x: decline",
            "quick verdict",
            "start/finish review",
            "resolve thread",
            "edit own",
            "delete own",
            "1-5",
        ] {
            assert!(
                !text.contains(hidden),
                "unexpected {hidden} with help={help_open}"
            );
        }
        assert!(text.contains("Overview"));
        assert!(text.contains("Reviewers"));
        if help_open {
            assert!(text.contains("1-4"));
        }
    }
    state.ui.detail.help_open = false;
    state.store.capabilities.features.insert(Feature::Builds);
    let mut terminal = Terminal::new(TestBackend::new(150, 50)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(text.contains("Builds"));
    assert!(!text.contains("a: verdict"));
}

#[test]
fn compact_list_keeps_title_and_help_visible() {
    for width in [40, 70, 100, 140] {
        let mut state = fixture();
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains("Component migration"),
            "title lost at {width}"
        );
        assert!(text.contains("?: help"), "help lost at {width}");
        assert!(!text.contains("donate"));
    }
}

#[test]
fn help_scroll_reaches_last_action_in_small_terminal() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Overview,
    };
    state.ui.detail.help_open = true;
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal.draw(|f| render(f, &mut state)).unwrap();
    for _ in 0..8 {
        let action = key(&state, KeyCode::PageDown);
        state.ui.update(action, &state.store, state.screen);
    }
    terminal.draw(|f| render(f, &mut state)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(text.contains("quit"));
    assert!(text.contains("esc: close"));
    assert_eq!(
        state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
}

#[test]
fn compact_detail_tabs_always_show_the_active_tab() {
    for tab in DetailTab::ALL {
        let mut state = fixture();
        state.screen = Screen::Detail { pr_id: 42, tab };
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains(&format!("{}   h/l: tabs", tab.label())),
            "{tab:?}"
        );
    }
}

#[test]
fn multiline_editor_scrolls_to_cursor_and_keeps_controls_visible() {
    for (width, height) in [(100, 30), (40, 12)] {
        let mut state = fixture();
        state.screen = Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview,
        };
        state.ui.detail.editor.draft = Some(CommentDraft {
            target: CommentTarget::Pr,
            text: format!("{}last line 🦀", "earlier line\n".repeat(30)),
        });
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains("PR comment"));
        assert!(text.contains("last line"));
        assert!(text.contains("newline"));
        assert!(text.contains("discard"));
        let cursor = terminal.get_cursor_position().unwrap();
        assert!(cursor.x < width - 2 && cursor.y < height - 3);
    }
}

#[test]
fn link_shortcuts_target_selected_pr_and_never_escape_editor_or_help() {
    use crate::app::action::LinkAction;
    let mut state = fixture();
    if let LoadState::Loaded(prs) = &mut state.store.cache.prs {
        let mut second = prs[0].clone();
        second.id = 43;
        second.title = "Different PR".into();
        prs.push(second);
    }
    state.ui.list.search.query = "Different".into();
    assert!(matches!(
        key(&state, KeyCode::Char('y')),
        Action::PrLink {
            pr_id: 43,
            kind: LinkAction::Copy
        }
    ));
    for tab in DetailTab::ALL {
        state.screen = Screen::Detail { pr_id: 42, tab };
        assert!(matches!(
            key(&state, KeyCode::Char('o')),
            Action::PrLink {
                pr_id: 42,
                kind: LinkAction::Open
            }
        ));
    }
    state.ui.detail.help_open = true;
    assert!(
        key_to_action(
            &state,
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE)
        )
        .is_none()
    );
    state.ui.detail.help_open = false;
    state.ui.detail.editor.draft = Some(CommentDraft {
        target: CommentTarget::Pr,
        text: String::new(),
    });
    for ch in ['o', 'y'] {
        assert!(
            matches!(key(&state, KeyCode::Char(ch)), Action::Detail(DetailAction::CommentType(c)) if c == ch)
        );
    }
}

#[test]
fn missing_pr_link_hides_shortcuts_and_help_entries_in_both_screens() {
    for screen in [
        Screen::List,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview,
        },
    ] {
        let mut state = fixture();
        if let LoadState::Loaded(prs) = &mut state.store.cache.prs {
            prs[0].url = None;
        }
        state.screen = screen;
        for ch in ['o', 'y'] {
            assert!(
                key_to_action(&state, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                    .is_none()
            );
        }
        let action = key(&state, KeyCode::Char('?'));
        state.ui.update(action, &state.store, state.screen);
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(!text.contains("copy PR link"));
        assert!(!text.contains("open PR in browser"));
    }
}

#[test]
fn notice_replaces_entire_footer_and_normal_hints_return_afterward() {
    for screen in [
        Screen::List,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview,
        },
    ] {
        let mut state = fixture();
        state.screen = screen;
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        state.store.notice = Some(crate::app::store::Notice::new(
            "PR #42: link copied".into(),
            false,
        ));
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let footer: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 29)].symbol())
            .collect();
        assert_eq!(footer.trim(), "PR #42: link copied");
        state.store.notice = None;
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let footer: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 29)].symbol())
            .collect();
        assert!(footer.contains("?: help"));
        assert!(!footer.contains("link copied"));
    }
}
