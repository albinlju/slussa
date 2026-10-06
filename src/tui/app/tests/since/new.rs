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
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    // A merge of the target brought other.rs into the branch; the PR does not touch it.
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Ok(compared(&["src/main.rs", "other.rs"], Some(&[]))),
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
        Ok(compared(&[], Some(&[]))),
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

/// A reader with what is new open, and the PR's diff of the new head there.
fn reader_asking_what_is_new() -> App {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    app
}

#[tokio::test(flavor = "current_thread")]
async fn a_branch_that_moved_forward_shows_what_is_new_from_the_commit_that_was_read() {
    let mut app = reader_asking_what_is_new();
    compare_from(&mut app, "aaa111", &["src/main.rs"]);
    let text = screen_text(&mut app);
    assert!(
        text.contains("↻ new since you read it  aaa111 → bbb222"),
        "{text}"
    );
    assert!(!text.contains("rewritten"), "{text}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_branch_that_was_rewritten_says_the_compare_is_not_only_what_is_new() {
    let mut app = reader_asking_what_is_new();
    // The compare starts at the commit the two share, not at the one that was read.
    compare_from(&mut app, "ccc333", &["src/main.rs"]);
    let text = screen_text(&mut app);
    assert!(text.contains("↻ rewritten since you read it"), "{text}");
    assert!(text.contains("from ccc333, not aaa111"), "{text}");
    assert!(text.contains("not what was dropped"), "{text}");
    assert!(!text.contains("↻ new since you read it  aaa111"), "{text}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_branch_that_was_reset_says_so_instead_of_that_nothing_is_new() {
    let mut app = reader_asking_what_is_new();
    // The head now is under the commit that was read, so the compare starts at it.
    compare_from(&mut app, "bbb222", &[]);
    let text = screen_text(&mut app);
    assert!(
        text.contains("↻ reset since you read it  aaa111 → back to bbb222"),
        "{text}"
    );
    assert!(
        text.contains("The branch was reset to a commit older"),
        "{text}"
    );
    assert!(!text.contains("Nothing new"), "{text}");
}

fn kept(app: &App) -> Vec<&str> {
    let LoadState::Loaded(diff) = what_is_new(app) else {
        panic!("read");
    };
    diff.files.iter().map(|file| file.path.as_str()).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_the_pr_put_back_as_the_target_has_it_is_new_and_is_kept() {
    let mut app = returning_reader("aaa111", "bbb222");
    // The PR touched guard.rs when it was read, and no longer does: it was put back.
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    compare_arrives_of(
        &mut app,
        &["src/main.rs", "src/guard.rs", "other.rs"],
        Some(&["src/main.rs", "src/guard.rs"]),
    );
    // other.rs came with a merge of the target, in a file the PR never touched.
    assert_eq!(kept(&app), ["src/main.rs", "src/guard.rs"]);
}

#[tokio::test(flavor = "current_thread")]
async fn nothing_is_left_out_when_what_the_pr_touched_before_is_not_known() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    compare_arrives_of(&mut app, &["src/main.rs", "src/guard.rs", "other.rs"], None);
    assert_eq!(kept(&app), ["src/main.rs", "src/guard.rs", "other.rs"]);
}

#[test]
fn the_commit_the_pr_is_against_is_taken_from_the_diff_of_the_head_asked_for() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &[]);
    assert_eq!(
        app.target_at(PrId(42), &oid("bbb222")),
        Some(oid("0ba5e0")),
        "the diff of that head says it"
    );
    assert_eq!(app.target_at(PrId(42), &oid("ccc333")), None);
}

#[tokio::test(flavor = "current_thread")]
async fn a_compare_that_answers_after_the_list_says_the_branch_moved_on_is_not_kept() {
    let mut app = returning_reader("aaa111", "bbb222");
    diff_of_files(&mut app, "bbb222", &["src/main.rs"]);
    press(&mut app, KeyCode::Char('w'));
    // A push while the compare is on its way: the list is read again and says so.
    branch_at(&mut app, "ccc333");
    compare_arrives(&mut app, &["src/main.rs"]);

    let LoadState::Failed(error) = what_is_new(&app) else {
        panic!("not kept");
    };
    assert!(error.user_message().contains("moved again"));
    assert_eq!(read_head(&app), Some(&oid("aaa111")));
    // Out and in again is what is new to where the branch is now.
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('w'));
    assert!(
        app.state.store.fetches.contains(&WHOLE),
        "the diff of ccc333 first"
    );
}
