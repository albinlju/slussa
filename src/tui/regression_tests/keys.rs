//! Where a key goes: screen, focused child, dialog or editor.

use super::support::*;

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
        Action::Detail(DetailAction::Editor(EditorAction::Type('q')))
    ));
    assert!(matches!(
        key(&state, KeyCode::Enter),
        Action::Detail(DetailAction::Editor(EditorAction::Type('\n')))
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
        Action::Detail(DetailAction::Confirm(ConfirmAction::Accept))
    ));
    assert!(matches!(
        key(&state, KeyCode::Esc),
        Action::Detail(DetailAction::Confirm(ConfirmAction::Close))
    ));
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
        Action::Effect(Effect::PrLink {
            pr_id: 43,
            kind: LinkAction::Copy
        })
    ));
    for tab in DetailTab::ALL {
        state.screen = Screen::Detail { pr_id: 42, tab };
        assert!(matches!(
            key(&state, KeyCode::Char('o')),
            Action::Effect(Effect::PrLink {
                pr_id: 42,
                kind: LinkAction::Open
            })
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
            matches!(key(&state, KeyCode::Char(ch)), Action::Detail(DetailAction::Editor(EditorAction::Type(c))) if c == ch)
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
