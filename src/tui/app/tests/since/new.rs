//! What is new: which files it holds, in which order it is read, and what it
//! says when it cannot be told.

use super::support::*;

#[tokio::test(flavor = "current_thread")]
async fn no_comment_is_made_on_what_is_new_and_leaving_the_tab_closes_it() {
    let mut app = returning_reader("aaa111", "bbb222");
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Char('c'));
    assert!(!app.state.ui.detail.modal_open(), "no editor on its lines");

    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(
        DetailTab::Overview,
    ))));
    assert!(app.state.ui.detail.since.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn esc_in_what_is_new_goes_back_to_the_whole_diff_not_to_the_list() {
    let mut app = returning_reader("aaa111", "bbb222");
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.since.is_none());
    assert!(matches!(
        app.state.screen,
        Screen::Detail {
            tab: DetailTab::Diff,
            ..
        }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_the_new_head_added_is_new_though_the_diff_on_screen_is_of_the_older_head() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "aaa111", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    diff_arrives(&mut app, "bbb222", &["src/main.rs", "src/added.rs"]);
    // A merge of the target brought other.rs; added.rs came with the new head.
    compare_arrives(&mut app, &["src/main.rs", "src/added.rs", "other.rs"]);

    let LoadState::Loaded(diff) = what_is_new(&app) else {
        panic!("read");
    };
    let paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/main.rs", "src/added.rs"]);
    assert_eq!(read_head(&app), Some(&oid("bbb222")));
}

#[tokio::test(flavor = "current_thread")]
async fn a_compare_that_arrives_beside_the_diff_of_an_older_head_is_not_kept_and_not_read() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "aaa111", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    compare_arrives(&mut app, &["src/main.rs", "src/added.rs"]);

    assert!(
        matches!(what_is_new(&app), LoadState::Failed(_)),
        "which files are the PR's at the new head is not known"
    );
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    assert!(screen_text(&mut app).contains(MARK));
}

#[tokio::test(flavor = "current_thread")]
async fn a_branch_that_moved_again_while_what_is_new_was_read_says_so() {
    let mut app = returning_reader("aaa111", "bbb222");
    press(&mut app, KeyCode::Char('w'));
    app.state.store.fetches.clear();
    diff_arrives(&mut app, "ccc333", &["src/main.rs"]);

    let LoadState::Failed(error) = what_is_new(&app) else {
        panic!("not read");
    };
    assert!(error.user_message().contains("moved again"));
    assert!(screen_text(&mut app).contains("moved again"));
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    let compare = FetchKey::Pr(PrResource::RangeDiff(range("aaa111", "bbb222")), PrId(42));
    assert!(!app.state.store.fetches.contains(&compare));
    // The list is read again, since `w` goes by where it says the branch is.
    let list = FetchKey::Prs(crate::domain::pr::PrGroup::Open);
    assert!(app.state.store.fetches.contains(&list));
}

#[tokio::test(flavor = "current_thread")]
async fn what_is_new_says_when_the_diff_of_the_pr_could_not_be_read_and_f_asks_again() {
    let mut app = returning_reader("aaa111", "bbb222");
    press(&mut app, KeyCode::Char('w'));
    app.apply_result(TaskResult::Read(Read::Diff(
        PrId(42),
        Err(failed("offline")),
    )));
    let LoadState::Failed(error) = what_is_new(&app) else {
        panic!("not read");
    };
    assert!(error.user_message().contains("offline"));
    assert_eq!(read_head(&app), Some(&oid("aaa111")));

    press(&mut app, KeyCode::Char('F'));
    assert!(app.state.store.fetches.contains(&WHOLE), "F reads it again");
    diff_arrives(&mut app, "bbb222", &[]);
    let compare = FetchKey::Pr(PrResource::RangeDiff(range("aaa111", "bbb222")), PrId(42));
    assert!(app.state.store.fetches.contains(&compare));
}

#[tokio::test(flavor = "current_thread")]
async fn leaving_what_is_new_that_could_not_be_read_keeps_what_was_read() {
    let mut app = returning_reader("aaa111", "bbb222");
    // The whole diff is of the branch as it is, as it is once it has been read again.
    diff_of_files(&mut app, "bbb222", &[]);
    press(&mut app, KeyCode::Char('w'));
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Err(failed("HTTP 403: rate limited")),
    )));
    // Out by either way, `w` or `esc`: the commit that was read is not lost.
    press(&mut app, KeyCode::Char('w'));
    assert!(app.state.ui.detail.since.is_none());
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    assert!(
        screen_text(&mut app).contains(MARK),
        "and it can be asked for again"
    );

    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(read_head(&app), Some(&oid("aaa111")));

    // Asking again reads it again, since the first time failed.
    press(&mut app, KeyCode::Char('w'));
    let wanted = FetchKey::Pr(PrResource::RangeDiff(range("aaa111", "bbb222")), PrId(42));
    assert!(app.state.store.fetches.contains(&wanted));
}

#[tokio::test(flavor = "current_thread")]
async fn f_asks_again_for_what_is_new_that_could_not_be_read() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &[]);
    press(&mut app, KeyCode::Char('w'));
    let wanted = FetchKey::Pr(PrResource::RangeDiff(range("aaa111", "bbb222")), PrId(42));
    app.state.store.fetches.remove(&wanted);
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Err(failed("offline")),
    )));
    assert!(!app.state.store.fetches.contains(&wanted));
    press(&mut app, KeyCode::Char('F'));
    assert!(
        app.state.store.fetches.contains(&wanted),
        "F reads it again"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn what_is_new_is_what_is_new_in_the_files_of_the_pr() {
    use crate::domain::diff::FileDiff;
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    let file = |path: &str| FileDiff {
        path: path.into(),
        hunks: vec![],
    };
    // A merge of the target brought other.rs into the branch; the PR does not touch it.
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Ok(Diff {
            revision: None,
            files: vec![file("src/main.rs"), file("other.rs")],
        }),
    )));
    let kept = &app.state.store.cache.details[&PrId(42)].range_diffs[&range("aaa111", "bbb222")];
    let LoadState::Loaded(diff) = kept else {
        panic!("read");
    };
    let paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/main.rs"]);
}

#[tokio::test(flavor = "current_thread")]
async fn nothing_new_in_the_files_of_the_pr_says_so_and_what_it_may_mean() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &[]);
    press(&mut app, KeyCode::Char('w'));
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Ok(Diff {
            revision: None,
            files: vec![],
        }),
    )));
    let text = screen_text(&mut app);
    assert!(
        text.contains("Nothing new in the files of this PR"),
        "{text}"
    );
    assert!(
        text.contains("reset"),
        "it says what an empty compare can be"
    );
}
