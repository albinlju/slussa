//! Dialogs and the comment editor: content, footers, scrolling and choices.

use super::support::*;

#[test]
fn extracted_dialogs_render_and_keep_their_key_bindings() {
    for dialog in ["confirm", "review", "merge", "error", "help"] {
        let mut state = fixture();
        state.screen = Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Overview,
        };
        let (label, close) = match dialog {
            "confirm" => {
                state.ui.detail.overlay =
                    Some(Overlay::Confirm(ConfirmDialog::new(ConfirmKind::Decline)));
                (
                    "Close / decline this PR?",
                    DetailAction::Confirm(ConfirmAction::Close),
                )
            }
            "review" => {
                state.ui.detail.overlay = Some(Overlay::Review(ReviewDialog::default()));
                ("Submit review", DetailAction::Review(ReviewAction::Close))
            }
            "merge" => {
                state.ui.detail.overlay = Some(Overlay::Merge(MergeDialog::default()));
                ("Merge this PR", DetailAction::Merge(MergeAction::Close))
            }
            "error" => {
                state.store.errors.insert(PrId(42), "Request failed".into());
                ("Request failed", DetailAction::Error(ErrorAction::Dismiss))
            }
            "help" => {
                state.ui.detail.overlay = Some(Overlay::Help(HelpDialog::default()));
                ("Help", DetailAction::Nav(NavAction::ToggleHelp))
            }
            other => panic!("no such dialog in this test: {other}"),
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
                Action::Detail(DetailAction::Review(ReviewAction::Select))
            ));
        }
        if dialog == "merge" {
            assert!(matches!(
                key(&state, KeyCode::Enter),
                Action::Detail(DetailAction::Merge(MergeAction::Select))
            ));
        }
    }
}

#[test]
fn dialog_instances_own_selection_and_respect_available_choices() {
    use crate::{
        domain::{pr::MergeStrategy, review::ReviewVerdict},
        tui::ui::{component::Component, screens::pr_detail::dialogs::review::ReviewContext},
    };
    let mut confirm = ConfirmDialog::new(ConfirmKind::Decline);
    let other = ConfirmDialog::new(ConfirmKind::Decline);
    confirm.update(ConfirmAction::Move(-1), &());
    assert_eq!(confirm.accepted(), Some(ConfirmKind::Decline));
    assert_eq!(other.accepted(), None);

    let ctx = ReviewContext {
        options: vec![
            (ReviewVerdict::Approve, Some("your PR")),
            (ReviewVerdict::Comment, None),
        ],
        pending: None,
    };
    let mut review = ReviewDialog::new(&ctx);
    assert_eq!(review.selected(&ctx), Some(ReviewVerdict::Comment));
    review.update(ReviewAction::Move(-1), &ctx);
    assert_eq!(review.selected(&ctx), None);

    let strategies = [MergeStrategy::Merge, MergeStrategy::Rebase];
    let mut merge = MergeDialog::default();
    merge.update(MergeAction::Move(99), &strategies.as_slice());
    assert_eq!(merge.selected(&strategies), Some(MergeStrategy::Rebase));
}

#[test]
fn multiline_editor_scrolls_to_cursor_and_keeps_controls_visible() {
    for (width, height) in [(100, 30), (40, 12)] {
        let mut state = fixture();
        state.screen = Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Overview,
        };
        state.ui.detail.editor = CommentEditor::start(
            CommentTarget::Pr,
            format!("{}last line 🦀", "earlier line\n".repeat(30)),
        );
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
fn dialog_footers_and_review_choices_remain_visible_with_large_queue() {
    use crate::tui::app::reviews::{CommentAnchor, PendingComment, PendingReview};
    for (width, height) in [(100, 30), (40, 12), (24, 8)] {
        for kind in ["review", "merge", "confirm"] {
            let mut state = fixture();
            state.screen = Screen::Detail {
                pr_id: PrId(42),
                tab: DetailTab::Overview,
            };
            state.store.reviews.insert(
                PrId(42),
                PendingReview {
                    comments: (0..100)
                        .map(|_| PendingComment {
                            anchor: CommentAnchor {
                                revision: None,
                                path: "src/main.rs".into(),
                                line: 2,
                                removed: false,
                            },
                            text: "Long queued comment".into(),
                        })
                        .collect(),
                    submitted_summary: None,
                },
            );
            let expected = match kind {
                "review" => {
                    state.ui.detail.overlay = Some(Overlay::Review(ReviewDialog::default()));
                    "Approve"
                }
                "merge" => {
                    state.ui.detail.overlay = Some(Overlay::Merge(MergeDialog::default()));
                    "Merge commit"
                }
                _ => {
                    state.ui.detail.overlay =
                        Some(Overlay::Confirm(ConfirmDialog::new(ConfirmKind::Decline)));
                    "Yes"
                }
            };
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|f| render(f, &mut state)).unwrap();
            let text = rendered_text(&terminal);
            assert!(
                text.contains(expected),
                "{kind} selection missing at {width}x{height}"
            );
            assert!(text.contains(if kind == "review" {
                "Enter submit"
            } else if kind == "merge" {
                "Enter merge"
            } else {
                "Enter select"
            }));
            assert!(text.contains("Esc cancel"));
            assert!(!text.contains("v: finish"));
            local_key(&mut state, KeyCode::Esc);
            assert!(!state.ui.detail.modal_open());
            assert_eq!(state.store.reviews[&PrId(42)].comments.len(), 100);
        }
    }
}

#[test]
fn long_error_scrolls_without_dismissing_or_acting_on_the_pr() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    state.store.errors.insert(
        PrId(42),
        format!("{}END_OF_ERROR", "Long failure details.\n".repeat(40)),
    );
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal.draw(|f| render(f, &mut state)).unwrap();
    assert!(
        key_to_action(
            &state,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)
        )
        .is_none()
    );
    for _ in 0..40 {
        local_key(&mut state, KeyCode::PageDown);
    }
    terminal.draw(|f| render(f, &mut state)).unwrap();
    let text = rendered_text(&terminal);
    assert!(text.contains("END_OF_ERROR"));
    assert!(text.contains("Esc / Enter close"));
    assert!(state.ui.detail.review_picker().is_none());
    assert!(matches!(
        key(&state, KeyCode::Esc),
        Action::Detail(DetailAction::Error(ErrorAction::Dismiss))
    ));
}

#[test]
fn review_preview_shows_full_comments_without_submitting() {
    use crate::tui::app::reviews::CommentAnchor;
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    state.store.reviews.insert(
        PrId(42),
        crate::tui::app::reviews::PendingReview {
            comments: (0..4)
                .map(|index| crate::tui::app::reviews::PendingComment {
                    anchor: CommentAnchor {
                        revision: None,
                        path: format!("src/file{index}.rs"),
                        line: 1,
                        removed: false,
                    },
                    text: format!(
                        "{}END_COMMENT_{index}",
                        "More context to review.\n".repeat(12)
                    ),
                })
                .collect(),
            submitted_summary: None,
        },
    );
    local_key(&mut state, KeyCode::Char('v'));
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("inspect comments"));
    local_key(&mut state, KeyCode::Tab);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("not published"));
    assert!(key_to_action(&state, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).is_none());
    for _ in 0..100 {
        local_key(&mut state, KeyCode::Char('j'));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
    }
    assert!(rendered_text(&terminal).contains("END_COMMENT_3"));
    local_key(&mut state, KeyCode::Tab);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("Approve"));
    local_key(&mut state, KeyCode::Esc);
    assert_eq!(state.store.reviews[&PrId(42)].comments.len(), 4);
    assert!(state.store.operations.is_empty());
}

#[test]
fn editor_distinguishes_queued_comments_from_direct_publication() {
    use crate::tui::app::reviews::CommentAnchor;
    use crate::tui::ui::components::comment_editor::CommentEditor;
    for (target, expected) in [
        (
            CommentTarget::Line(CommentAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: 1,
                removed: false,
            }),
            "add to review",
        ),
        (CommentTarget::Pr, "post comment"),
        (CommentTarget::Reply(CommentId(10)), "post reply"),
        (
            CommentTarget::Review {
                verdict: crate::domain::review::ReviewVerdict::Comment,
            },
            "submit review",
        ),
    ] {
        for width in [40, 100] {
            let mut editor = CommentEditor::start(target.clone(), "Draft".into());
            let mut terminal = Terminal::new(TestBackend::new(width, 16)).unwrap();
            terminal
                .draw(|frame| {
                    let view = EditorView {
                        sending: false,
                        review_active: true,
                        context: None,
                    };
                    editor.render(frame, frame.area(), &view);
                })
                .unwrap();
            assert!(rendered_text(&terminal).contains(expected));
        }
    }
}

#[test]
fn mutation_dialogs_show_pr_and_target_even_with_a_long_source_branch() {
    for width in [40, 100] {
        for merge in [false, true] {
            let mut state = fixture();
            state.screen = Screen::Detail {
                pr_id: PrId(42),
                tab: DetailTab::Overview,
            };
            if let LoadState::Loaded(prs) = &mut state.store.cache.prs {
                prs[0].source_branch = "feature/".repeat(20);
            }
            if merge {
                state.ui.detail.overlay = Some(Overlay::Merge(MergeDialog::default()));
            } else {
                state.ui.detail.overlay =
                    Some(Overlay::Confirm(ConfirmDialog::new(ConfirmKind::Decline)));
                assert!(state.ui.detail.confirm().unwrap().accepted().is_none());
            }
            let mut terminal = Terminal::new(TestBackend::new(width, 16)).unwrap();
            terminal.draw(|frame| render(frame, &mut state)).unwrap();
            let text = rendered_text(&terminal);
            assert!(text.contains("PR #42"));
            assert!(text.contains(if merge { "Into: main" } else { "Target: main" }));
            assert!(text.contains(if merge {
                "Enter merge"
            } else {
                "Closes without merging"
            }));
        }
    }
}

#[test]
fn resumed_editor_and_delete_dialog_identify_the_comment() {
    use crate::tui::ui::{component::Component, components::comment_editor::CommentEditor};
    for width in [40, 100] {
        let mut editor = CommentEditor::restored(CommentDraft {
            target: CommentTarget::Reply(CommentId(7)),
            text: "My draft".into(),
        });
        assert!(editor.resume());
        let mut terminal = Terminal::new(TestBackend::new(width, 16)).unwrap();
        terminal
            .draw(|frame| {
                let view = EditorView {
                    sending: false,
                    review_active: false,
                    context: Some("@alice: Original comment".into()),
                };
                editor.render(frame, frame.area(), &view);
            })
            .unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Resuming draft"));
        assert!(text.contains("@alice: Original comment"));
        assert!(text.contains("My draft"));
        assert!(text.contains("post reply"));
        let mut dialog = ConfirmDialog::new(ConfirmKind::DeleteComment(
            crate::domain::comment::CommentKey {
                id: CommentId(7),
                kind: crate::domain::comment::CommentKind::Review,
            },
        ))
        .with_preview("@alice: Original comment".into());
        let pr = PrSummary {
            label: "PR #42 · Component migration".into(),
            target_branch: "main",
            source_branch: "feature",
        };
        terminal
            .draw(|frame| dialog.render(frame, frame.area(), &pr))
            .unwrap();
        assert!(rendered_text(&terminal).contains("@alice: Original comment"));
    }
}
