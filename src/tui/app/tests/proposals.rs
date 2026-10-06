//! What agents proposed is drawn on the lines it is about, and the reader takes
//! it into a comment of their own or discards it.

use super::support::*;
use crate::{
    domain::{
        diff::DiffRevision,
        proposal::{Proposal, ProposalInput, Side},
    },
    local::{
        proposals::{ForPr, Proposals, import},
        seen::SeenStorage,
    },
    test_support::TempDir,
    tui::app::{proposals::ProposalsSource, seen::SeenFile},
};
use ratatui::{Terminal, backend::TestBackend};

fn proposal(head: &str, line: usize, body: &str) -> Proposal {
    Proposal::new(ProposalInput {
        head: head.into(),
        path: "src/main.rs".into(),
        line,
        side: Side::New,
        body: body.into(),
        id: None,
        agent: Some("reviewer".into()),
    })
    .unwrap()
}

/// The fixture's PR with its diff read at `abc123`, and these proposals on it.
fn app_with(proposals: Vec<Proposal>) -> App {
    let mut app = app();
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42))
        && let LoadState::Loaded(diff) = &mut data.diff
    {
        diff.revision = Some(DiffRevision {
            head: "abc123".into(),
            base: Some("0ba5e0".into()),
            commit: false,
        });
    }
    app.state.store.proposals = Proposals::of(
        PrId(42),
        ForPr {
            comments: proposals,
            summaries: vec![],
        },
    );
    detail(&mut app, DetailTab::Diff);
    app
}

fn screen_text(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &mut app.state))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// The cursor in the code on the proposal: from the folder in the file tree to
/// the file, into its code, over the two lines of the diff and on to it. The
/// pane is drawn between the keys, as it is for a reader: what the cursor can
/// stand on is what was drawn last.
fn on_the_proposal(app: &mut App) {
    press(app, KeyCode::Char('j'));
    app.apply(Action::Diff(DiffAction::EnterPane));
    for _ in 0..2 {
        screen_text(app);
        press(app, KeyCode::Char('j'));
    }
    screen_text(app);
}

fn handled(app: &App, proposal: &Proposal) -> bool {
    app.state.store.seen.is_handled(PrId(42), proposal)
}

#[test]
fn a_proposal_is_drawn_on_its_line_marked_as_an_agents_with_its_words() {
    let mut app = app_with(vec![proposal("abc123", 1, "This can panic.")]);
    let text = screen_text(&mut app);
    assert!(text.contains("This can panic."), "{text}");
    assert!(text.contains("[AI]") && text.contains("reviewer"), "{text}");
    assert!(text.contains("1 proposed by AI"), "the footer says so");
}

#[test]
fn a_proposal_written_against_another_commit_is_not_on_these_lines_but_is_counted() {
    let mut app = app_with(vec![
        proposal("abc123", 1, "On this commit."),
        proposal("def456", 1, "On an older one."),
    ]);
    let text = screen_text(&mut app);
    assert!(text.contains("On this commit."));
    assert!(!text.contains("On an older one."), "not on the wrong line");
    assert!(text.contains("for another commit"), "but counted");
}

#[tokio::test(flavor = "current_thread")]
async fn d_discards_a_proposal_and_it_is_not_shown_again() {
    let proposed = proposal("abc123", 1, "This can panic.");
    let dir = TempDir::new("proposals-seen");
    let (storage, _) = SeenStorage::open(dir.path(), "scope".into()).unwrap();
    let mut app = app_with(vec![proposed.clone()]);
    app.seen_file = SeenFile::Disk(storage);
    // Opened again so that the PR has a look to keep the decision in.
    detail(&mut app, DetailTab::Diff);
    on_the_proposal(&mut app);
    assert!(screen_text(&mut app).contains("d: discard"));
    press(&mut app, KeyCode::Char('d'));
    assert!(handled(&app, &proposed));
    assert!(!screen_text(&mut app).contains("This can panic."));

    // Another start reads the decision back: the same proposal stays discarded.
    app.seen_file = SeenFile::Unavailable;
    let (_storage, seen) = crate::local::seen::reopen(dir.path(), "scope").unwrap();
    assert!(seen.is_handled(PrId(42), &proposed));
}

#[tokio::test(flavor = "current_thread")]
async fn c_takes_a_proposal_into_an_editor_that_starts_from_its_words() {
    let proposed = proposal("abc123", 1, "This can panic.");
    let mut app = app_with(vec![proposed.clone()]);
    on_the_proposal(&mut app);
    assert!(screen_text(&mut app).contains("c: take as comment"));
    press(&mut app, KeyCode::Char('c'));
    assert!(
        app.state.ui.detail.editor.is_open(),
        "an editor on the line"
    );
    assert!(handled(&app, &proposed), "taken: not shown again");
    assert!(
        screen_text(&mut app).contains("This can panic."),
        "the words are in the editor"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_draft_in_progress_is_resumed_and_does_not_take_another_proposal() {
    let first = proposal("abc123", 1, "First.");
    let mut app = app_with(vec![first.clone()]);
    app.state.ui.detail.editor = ui::components::comment_editor::CommentEditor::start(
        CommentTarget::Pr,
        "my own draft".into(),
    );
    press(&mut app, KeyCode::Esc);
    on_the_proposal(&mut app);
    press(&mut app, KeyCode::Char('c'));
    assert!(
        !handled(&app, &first),
        "the draft is resumed, nothing taken"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_proposal_the_reader_dealt_with_stays_dealt_with_when_the_file_is_read_again() {
    let proposed = proposal("abc123", 1, "This can panic.");
    let mut app = app_with(vec![proposed.clone()]);
    app.state
        .store
        .seen
        .handle(PrId(42), &proposed, crate::domain::seen::How::Discarded);
    let dir = TempDir::new("proposals");
    import(
        dir.path(),
        "scope",
        PrId(42),
        crate::local::proposals::Batch {
            comments: vec![proposed, proposal("abc123", 2, "Another.")],
            summary: None,
        },
    )
    .unwrap();
    app.proposals_source = Some(ProposalsSource {
        root: dir.path().to_path_buf(),
        scope: "scope".into(),
    });
    press(&mut app, KeyCode::Char('F'));
    let text = screen_text(&mut app);
    assert!(
        !text.contains("This can panic."),
        "discarded stays discarded"
    );
    assert!(text.contains("1 proposed by AI"), "the new one is counted");
}

fn summary(head: &str, text: &str) -> crate::domain::proposal::Summary {
    crate::domain::proposal::Summary::new(head, text.into(), Some("reviewer".into())).unwrap()
}

/// The fixture's PR on the Overview, with these proposals and summaries.
fn overview_with(comments: Vec<Proposal>, summaries: Vec<crate::domain::proposal::Summary>) -> App {
    let mut app = app();
    app.state.store.proposals = Proposals::of(
        PrId(42),
        ForPr {
            comments,
            summaries,
        },
    );
    detail(&mut app, DetailTab::Overview);
    app
}

#[test]
fn what_is_waiting_is_said_on_every_tab_and_not_only_in_the_diff() {
    let mut app = overview_with(
        vec![proposal("abc123", 1, "One."), proposal("abc123", 2, "Two.")],
        vec![],
    );
    assert!(screen_text(&mut app).contains("2 proposed by AI (Diff tab)"));
    for tab in [
        DetailTab::Description,
        DetailTab::Commits,
        DetailTab::Builds,
    ] {
        detail(&mut app, tab);
        assert!(
            screen_text(&mut app).contains("2 proposed by AI (Diff tab)"),
            "{tab:?}"
        );
    }
    // Nothing to say when nothing is waiting.
    let mut none = overview_with(vec![], vec![]);
    assert!(!screen_text(&mut none).contains("proposed by AI"));
}

#[test]
fn the_summary_an_agent_handed_in_is_in_the_sidebar_with_what_is_left_to_decide() {
    let mut app = overview_with(
        vec![proposal("abc123", 1, "One.")],
        vec![summary(
            "abc123",
            "Two things deserve attention in this change.",
        )],
    );
    let text = screen_text(&mut app);
    assert!(text.contains("AI review"), "{text}");
    assert!(text.contains("[AI] reviewer"));
    assert!(text.contains("Two things deserve"), "the summary is read");
    assert!(text.contains("1 proposed · Diff tab"));
    assert!(
        !text.contains("older commit"),
        "it is of the commit the branch is at"
    );
}

#[test]
fn a_summary_of_an_older_commit_says_so_and_one_with_nothing_left_says_that() {
    let mut app = overview_with(vec![], vec![summary("def456", "All is well.")]);
    let text = screen_text(&mut app);
    assert!(text.contains("of an older commit"), "{text}");
    assert!(text.contains("Nothing left to decide"));
    // No summary and no proposals: no section at all.
    let mut empty = overview_with(vec![], vec![]);
    assert!(!screen_text(&mut empty).contains("AI review"));
}
