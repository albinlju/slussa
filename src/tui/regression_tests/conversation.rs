//! Threads in the Overview and the diff: selection, folding and targets.

use super::support::*;

#[test]
fn timeline_keeps_thread_selection_and_sidebar_is_responsive() {
    use crate::domain::comment::{Comment, CommentThread, ThreadAnchor};
    let mut state = fixture();
    state.store.current_user = "alice".into();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    let comments = (10..12)
        .map(|id| Comment {
            id: Some(CommentId(id)),
            author: User {
                username: "alice".into(),
            },
            account: crate::domain::user::AccountKind::Person,
            content: format!("Comment {id}"),
            created: chrono::Utc::now(),
            reactions: vec![],
            reply_to: Some(CommentId(10)),
        })
        .collect();
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![],
        events: vec![],
        threads: vec![CommentThread {
            comments,
            reply_to: Some(CommentId(10)),
            anchor: Some(ThreadAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: Some(LineRef::New(1)),
                resolved: false,
                handle: Some(ThreadHandle::NodeId("thread-1".into())),
            }),
        }],
    });
    // Activity can arrive before the diff. Do not show cards that will grow
    // when snippets arrive; a failed diff must still let the discussion be read.
    let loaded_diff = std::mem::replace(
        &mut state.store.cache.details.get_mut(&PrId(42)).unwrap().diff,
        LoadState::Loading,
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    let loading = rendered_text(&terminal);
    assert!(loading.contains("Loading code context"));
    assert!(!loading.contains("Comment 10"));
    state.store.cache.details.get_mut(&PrId(42)).unwrap().diff =
        LoadState::Failed(crate::providers::FetchError::Network("offline".into()));
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("Comment 10"));
    state.store.cache.details.get_mut(&PrId(42)).unwrap().diff = loaded_diff;
    let mut preview = String::new();
    for width in [100, 80, 40] {
        let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        writeln!(
            preview,
            "{width}x30 Overview\n{}",
            terminal
                .backend()
                .buffer()
                .content
                .chunks(width as usize)
                .map(|row| row
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
        .unwrap();
        assert_eq!(text.contains("Reviewers"), width >= 80);
        assert_eq!(state.ui.detail.overview.timeline.reply, Some(CommentId(10)));
        assert_eq!(
            state.detail_view().focused_thread().unwrap().handle,
            Some(ThreadHandle::NodeId("thread-1".into()))
        );
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tui/testdata/conversation.txt"
    );
    if std::env::var_os("SLUSSA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(path, &preview).unwrap();
    }
    assert_eq!(preview, std::fs::read_to_string(path).unwrap());
    assert_eq!(
        state.detail_view().editable_selected().unwrap().id,
        CommentId(10)
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
        CommentId(11)
    );
}

#[test]
fn compact_diff_switches_panels_and_keeps_file_selection() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Diff,
    };
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal.draw(|f| render(f, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("Files"));
    local_key(&mut state, KeyCode::Char('j'));
    let cursor = state.ui.detail.diff.cursor;
    local_key(&mut state, KeyCode::Enter);
    terminal.draw(|f| render(f, &mut state)).unwrap();
    let text = rendered_text(&terminal);
    assert!(text.contains("Code"));
    assert!(text.contains("old"));
    assert!(text.contains("esc: files"));
    assert!(state.ui.detail.diff.focused_anchor().is_some());
    local_key(&mut state, KeyCode::Esc);
    terminal.draw(|f| render(f, &mut state)).unwrap();
    assert_eq!(state.ui.detail.diff.focus, DiffFocus::Tree);
    assert_eq!(state.ui.detail.diff.cursor, cursor);
    assert!(rendered_text(&terminal).contains("Files"));
}

#[test]
fn unloaded_diff_cannot_reuse_a_previous_comment_target() {
    use crate::app::reviews::CommentAnchor;
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Diff,
    };
    state.ui.detail.diff.focus = DiffFocus::Pane;
    state.ui.detail.diff.pane = PaneNav {
        item_count: 30,
        matches: vec![3, 9],
        focused: Some(FocusedNav {
            anchor: CommentAnchor {
                revision: None,
                path: "old.rs".into(),
                line: 20,
                removed: false,
            },
            target: NavTarget::Line,
            fold: None,
        }),
    };
    state.ui.detail.diff.pane_cursor = 5;
    state.ui.detail.diff.pane_search.query = "old".into();
    state.store.cache.details.get_mut(&PrId(42)).unwrap().diff =
        LoadState::Failed(crate::providers::FetchError::Network("offline".into()));
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal.draw(|f| render(f, &mut state)).unwrap();
    assert!(state.detail_view().comment_target().is_none());
    assert!(rendered_text(&terminal).contains("F: retry"));
    // Nor do the keys move over rows that are no longer on screen.
    for code in [KeyCode::Char('j'), KeyCode::Char('n'), KeyCode::PageDown] {
        local_key(&mut state, code);
        assert_eq!(state.ui.detail.diff.pane_cursor, 5, "{code:?}");
    }
    assert_eq!(state.ui.detail.diff.pane.item_count, 0);
    assert!(state.ui.detail.diff.pane.matches.is_empty());
}

#[test]
fn diff_fold_keeps_target_and_shows_the_next_action() {
    use crate::domain::comment::{Comment, CommentThread, ThreadAnchor};
    for width in [40, 100] {
        let mut state = fixture();
        state.screen = Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Diff,
        };
        state
            .store
            .cache
            .details
            .get_mut(&PrId(42))
            .unwrap()
            .activity = LoadState::Loaded(Activity {
            comments: vec![],
            events: vec![],
            threads: vec![CommentThread {
                comments: vec![Comment {
                    id: Some(CommentId(10)),
                    author: User {
                        username: "alice".into(),
                    },
                    account: crate::domain::user::AccountKind::Person,
                    content: "Keep the error context.".into(),
                    created: chrono::Utc::now(),
                    reactions: vec![],
                    reply_to: Some(CommentId(10)),
                }],
                reply_to: Some(CommentId(10)),
                anchor: Some(ThreadAnchor {
                    revision: None,
                    path: "src/main.rs".into(),
                    line: Some(LineRef::New(1)),
                    resolved: true,
                    handle: Some(ThreadHandle::NodeId("thread-10".into())),
                }),
            }],
        });
        let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        local_key(&mut state, KeyCode::Char('j'));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        local_key(&mut state, KeyCode::Enter);
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        for _ in 0..2 {
            local_key(&mut state, KeyCode::Char('j'));
            terminal.draw(|frame| render(frame, &mut state)).unwrap();
        }
        let folded = rendered_text(&terminal);
        assert!(
            folded.contains("› 1 comment · ✓ resolved"),
            "{width}: {folded}"
        );
        assert!(folded.contains("space: expand thread"));
        assert!(!folded.contains("Keep the error context."));
        assert_eq!(state.ui.detail.diff.focused_reply(), Some(CommentId(10)));
        local_key(&mut state, KeyCode::Char(' '));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let expanded = rendered_text(&terminal);
        assert!(expanded.contains("⌄ 1 comment · ✓ resolved"));
        assert!(expanded.contains("space: collapse thread"));
        assert!(expanded.contains("Keep the error context."));
        assert_eq!(state.ui.detail.diff.focused_reply(), Some(CommentId(10)));
        local_key(&mut state, KeyCode::Char(' '));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        assert_eq!(rendered_text(&terminal), folded);
    }
}

#[test]
fn overview_reveals_selected_reply_and_allows_scrolling_long_text() {
    use crate::domain::comment::{Comment, CommentThread};
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    let now = chrono::Utc::now();
    let comment = |id, text: String| Comment {
        id: Some(id),
        author: User {
            username: "alice".into(),
        },
        account: crate::domain::user::AccountKind::Person,
        content: text,
        created: now,
        reactions: vec![],
        reply_to: Some(CommentId(10)),
    };
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![comment(CommentId(20), "Another discussion".into())],
        events: vec![],
        threads: vec![CommentThread {
            comments: vec![
                comment(CommentId(10), "Long comment paragraph.\n\n".repeat(35)),
                comment(CommentId(11), "Selected reply is visible".into()),
            ],
            reply_to: Some(CommentId(10)),
            anchor: None,
        }],
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    // The standalone comment sorts first; move to the long thread.
    local_key(&mut state, KeyCode::Char('j'));
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("Long comment paragraph."));
    let scroll = state.ui.detail.overview.timeline.scroll;
    local_key(&mut state, KeyCode::PageDown);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(state.ui.detail.overview.timeline.scroll > scroll);
    let action = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
    )
    .unwrap();
    state.ui.update(action, &state.store, state.screen);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("Selected reply is visible"));
    assert_eq!(
        state.ui.detail.overview.timeline.selected.unwrap().id,
        Some(CommentId(11))
    );
    let stable = state.ui.detail.overview.timeline.scroll;
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert_eq!(state.ui.detail.overview.timeline.scroll, stable);
    let reply_row = |terminal: &Terminal<TestBackend>| {
        terminal
            .backend()
            .buffer()
            .content
            .chunks(60)
            .position(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .contains("Selected reply is visible")
            })
    };
    let before = reply_row(&terminal);
    if let LoadState::Loaded(activity) = &mut state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity
    {
        let mut newer = comment(CommentId(30), "New discussion arrived".into());
        newer.created = now + chrono::Duration::seconds(10);
        activity.comments.push(newer);
        activity.threads[0]
            .comments
            .insert(1, comment(CommentId(12), "Earlier reply inserted".into()));
    }
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert_eq!(
        state.ui.detail.overview.timeline.selected.unwrap().id,
        Some(CommentId(11))
    );
    assert_eq!(state.ui.detail.overview.timeline.cursor, 2);
    assert_eq!(state.ui.detail.overview.timeline.sub, 2);
    assert_eq!(before, reply_row(&terminal));
    if let LoadState::Loaded(activity) = &mut state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity
    {
        activity.threads[0]
            .comments
            .retain(|c| c.id != Some(CommentId(11)));
    }
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert_ne!(
        state.ui.detail.overview.timeline.selected.unwrap().id,
        Some(CommentId(11))
    );
}

#[test]
fn a_thread_taller_than_the_diff_pane_is_shown_from_its_first_row() {
    use crate::domain::comment::{Comment, CommentThread, ThreadAnchor};
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Diff,
    };
    let paragraphs: Vec<String> = (1..=30).map(|n| format!("Paragraph {n}.")).collect();
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![],
        events: vec![],
        threads: vec![CommentThread {
            comments: vec![Comment {
                id: Some(CommentId(10)),
                author: User {
                    username: "alice".into(),
                },
                account: crate::domain::user::AccountKind::Person,
                content: format!("TOP_OF_THREAD\n\n{}", paragraphs.join("\n\n")),
                created: chrono::Utc::now(),
                reactions: vec![],
                reply_to: Some(CommentId(10)),
            }],
            reply_to: Some(CommentId(10)),
            anchor: Some(ThreadAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: Some(LineRef::New(1)),
                resolved: false,
                handle: Some(ThreadHandle::NodeId("thread-10".into())),
            }),
        }],
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    // Into the file's code, then down to the thread under its line.
    local_key(&mut state, KeyCode::Char('j'));
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    local_key(&mut state, KeyCode::Enter);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    for _ in 0..2 {
        local_key(&mut state, KeyCode::Char('j'));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
    }
    assert_eq!(
        state.ui.detail.diff.focused_reply(),
        Some(CommentId(10)),
        "on the thread"
    );

    let text = rendered_text(&terminal);
    assert!(text.contains("TOP_OF_THREAD"), "{text}");
    assert!(!text.contains("Paragraph 30."), "it does not fit: {text}");
}

#[test]
fn a_reply_being_written_names_the_comment_it_answers() {
    use crate::domain::comment::{Comment, CommentThread};
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .activity = LoadState::Loaded(Activity {
        comments: vec![],
        events: vec![],
        threads: vec![CommentThread {
            comments: vec![Comment {
                id: Some(CommentId(10)),
                author: User {
                    username: "alice".into(),
                },
                account: crate::domain::user::AccountKind::Person,
                content: "Why this name?\nSecond line".into(),
                created: chrono::Utc::now(),
                reactions: vec![],
                reply_to: Some(CommentId(10)),
            }],
            reply_to: Some(CommentId(10)),
            anchor: None,
        }],
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(!rendered_text(&terminal).contains("@alice: Why this name?"));

    state.ui.detail.editor =
        CommentEditor::start(CommentTarget::Reply(CommentId(10)), "Because".into());
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    let text = rendered_text(&terminal);
    assert!(text.contains("@alice: Why this name?"), "{text}");
    assert!(!text.contains("@alice: Why this name?Second"), "{text}");
    assert!(text.contains("Because"), "{text}");

    // A new comment answers nothing, so no line is shown above it.
    state.ui.detail.editor = CommentEditor::start(CommentTarget::Pr, "Standalone".into());
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(!rendered_text(&terminal).contains("@alice: Why this name?"));
}
