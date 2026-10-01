//! Draft recovery across restarts, and the journal written before a write.

use super::support::*;

fn recovery_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "slussa-recovery-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn attach_recovery(app: &mut App, root: &std::path::Path) {
    let (storage, snapshot) = crate::app::drafts::reopen(root, "test-repo/reviewer").unwrap();
    app.restore_drafts(storage, snapshot);
}

#[test]
fn restart_restores_closed_editor_and_discard_removes_it_from_disk() {
    let root = recovery_root("editor");
    let mut first = app();
    attach_recovery(&mut first, &root);
    detail(&mut first, DetailTab::Overview);
    press(&mut first, KeyCode::Char('c'));
    first.apply(Action::Paste("first\r\nsecond 🦀".into()));
    press(&mut first, KeyCode::Enter);
    assert!(first.state.store.operations.is_empty());
    press(&mut first, KeyCode::Esc);
    drop(first);
    let mut second = app();
    attach_recovery(&mut second, &root);
    detail(&mut second, DetailTab::Description);
    assert!(!second.state.ui.detail.editor.is_open());
    press(&mut second, KeyCode::Char('c'));
    assert!(second.state.ui.detail.editor.is_open());
    assert_eq!(
        second.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "first\nsecond 🦀\n"
    );
    second.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
    press(&mut second, KeyCode::Enter);
    drop(second);
    let mut third = app();
    attach_recovery(&mut third, &root);
    detail(&mut third, DetailTab::Description);
    assert!(third.state.ui.detail.editor.draft.is_none());
    drop(third);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn interrupted_send_is_journaled_and_success_clears_recovery_data() {
    let root = recovery_root("send");
    let mut first = app();
    attach_recovery(&mut first, &root);
    detail(&mut first, DetailTab::Overview);
    press(&mut first, KeyCode::Char('c'));
    first.apply(Action::Paste("send me".into()));
    send_comment(&mut first);
    assert!(first.state.store.operations.contains_key(&42));
    drop(first);
    let mut second = app();
    attach_recovery(&mut second, &root);
    assert!(second.state.store.operations.is_empty());
    assert!(second.state.store.errors[&42].contains("may have reached"));
    detail(&mut second, DetailTab::Overview);
    press(&mut second, KeyCode::Esc); // acknowledge interrupted request notice
    press(&mut second, KeyCode::Char('c'));
    send_comment(&mut second);
    finish_write(&mut second, 42, Ok(()));
    drop(second);
    let mut third = app();
    attach_recovery(&mut third, &root);
    detail(&mut third, DetailTab::Overview);
    assert!(third.state.ui.detail.editor.draft.is_none());
    assert!(third.state.store.uncertain_submissions.is_empty());
    assert!(third.state.store.errors.is_empty());
    drop(third);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn journal_failure_prevents_remote_submission_and_keeps_editor() {
    let root = recovery_root("failure");
    let mut app = app();
    attach_recovery(&mut app, &root);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    app.apply(Action::Paste("keep me".into()));
    let file = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    std::fs::create_dir(file.with_extension("tmp")).unwrap();
    // No runtime: a remote spawn here would panic, so this verifies the boundary.
    send_comment(&mut app);
    assert!(app.state.store.operations.is_empty());
    assert_eq!(
        app.state.ui.detail.editor.draft.as_ref().unwrap().text,
        "keep me"
    );
    assert!(app.state.store.errors[&42].starts_with("Not sent:"));
    assert!(
        std::fs::read(&file)
            .unwrap()
            .windows(7)
            .any(|s| s == b"keep me")
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

fn draft_file(root: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap()
}

#[test]
fn typing_is_not_written_per_key_and_the_next_save_or_other_action_writes_it() {
    let root = recovery_root("typing");
    let mut app = app();
    attach_recovery(&mut app, &root);
    detail(&mut app, DetailTab::Overview);
    press(&mut app, KeyCode::Char('c'));
    let file = draft_file(&root);
    let on_disk = || String::from_utf8(std::fs::read(&file).unwrap()).unwrap();
    let opened = on_disk();

    for c in "typed".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Left);
    assert_eq!(on_disk(), opened, "no write per key");
    assert!(app.drafts_dirty);

    // What the event loop does every `DRAFT_SAVE_INTERVAL` while text is unsaved.
    assert!(app.save_drafts());
    assert!(!app.drafts_dirty);
    assert!(on_disk().contains("\"type\""), "{}", on_disk());

    // Anything other than a keystroke is written at once, with what was typed.
    press(&mut app, KeyCode::Char('!'));
    assert!(on_disk().contains("\"type\""), "{}", on_disk());
    app.apply(Action::Paste("pasted".into()));
    assert!(on_disk().contains("typ!pastede"), "{}", on_disk());
    press(&mut app, KeyCode::Char('?'));
    press(&mut app, KeyCode::Esc);
    assert!(!app.drafts_dirty);
    assert!(on_disk().contains("typ!pasted?e"), "{}", on_disk());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
