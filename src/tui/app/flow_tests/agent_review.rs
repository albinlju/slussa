//! `A`: the PR's text goes to the configured command, and what it answers is kept
//! as proposals. The command here is a shell script, and `gh` is the fake.

use super::support::*;
use crate::{test_support::TempDir, tui::app::proposals::ProposalsSource};

const DIFF: &str = "diff --git a/src/main.rs b/src/main.rs\n\
--- a/src/main.rs\n\
+++ b/src/main.rs\n\
@@ -1 +1 @@\n\
-old\n\
+new\n";

fn pr_json(head: &str) -> String {
    json!({"head": {"sha": head}, "base": {"sha": "0ba5e0"}}).to_string()
}

fn fake_gh() -> FakeGh {
    FakeGh::new()
        .on("pr diff 42", DIFF)
        .on("pulls/42", &pr_json("abc123"))
}

/// The fixture's PR 42, asked about with `script` as the agent.
fn app_asking(script: &str, dir: &TempDir) -> App {
    let mut app = app();
    app.state = crate::tui::ui::regression_tests::fixture();
    app.state.store.agent_review = vec!["sh".into(), "-c".into(), script.into()];
    app.proposals_source = Some(ProposalsSource {
        root: dir.path().join("proposals"),
        scope: "scope".into(),
    });
    app
}

fn notice(app: &App) -> String {
    app.state
        .store
        .notice
        .as_ref()
        .map(|notice| notice.message.clone())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_agent_is_given_the_pr_and_its_answer_is_kept_as_proposals_on_lines_of_the_diff() {
    let dir = TempDir::new("agent-review");
    let prompt = dir.path().join("prompt.txt");
    // Wrapped in a fence and a sentence, as a model does.
    let script = format!(
        "cat > '{}'; printf 'Here you go:\\n```json\\n%s\\n```\\n' '{}'",
        prompt.display(),
        r#"{"summary": "One thing.", "comments": [
            {"path": "src/main.rs", "line": 1, "body": "This can panic.", "id": "F1"},
            {"path": "src/main.rs", "line": 9, "body": "Not in the diff."}]}"#
            .replace('\n', " ")
    );
    let _gh = fake_gh().install();
    let mut app = app_asking(&script, &dir);

    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    assert!(
        app.state.store.fetches.iter().any(|key| matches!(
            key,
            FetchKey::Pr(crate::tui::app::store::PrResource::AgentReview, PrId(42))
        )),
        "the review is running"
    );
    settle(&mut app).await;

    let held = app
        .state
        .store
        .proposals
        .for_pr(PrId(42))
        .expect("proposals");
    assert_eq!(held.comments.len(), 1, "the one on a line of the diff");
    assert_eq!(held.comments[0].body(), "This can panic.");
    assert_eq!(
        held.comments[0].head().as_str(),
        "abc123",
        "slussa's head, not the agent's"
    );
    assert_eq!(held.comments[0].agent(), Some("sh"));
    assert_eq!(held.summaries.len(), 1);
    let text = notice(&app);
    assert!(
        text.contains("proposed 1 comment") && text.contains("1 not on a line"),
        "{text}"
    );
    assert!(app.state.store.errors.is_empty());

    let given = std::fs::read_to_string(prompt).unwrap();
    for wanted in [
        "Component migration",
        "commit abc123",
        "+new",
        "feature -> main",
    ] {
        assert!(given.contains(wanted), "the agent was not told {wanted:?}");
    }
}

#[tokio::test]
async fn a_review_that_fails_is_the_pr_s_error_and_proposes_nothing() {
    let dir = TempDir::new("agent-review");
    let _gh = fake_gh().install();
    let mut app = app_asking("echo it broke >&2; exit 1", &dir);
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    settle(&mut app).await;
    let error = app
        .state
        .store
        .errors
        .get(&PrId(42))
        .cloned()
        .unwrap_or_default();
    assert!(
        error.contains("The review failed") && error.contains("it broke"),
        "{error}"
    );
    assert!(app.state.store.proposals.for_pr(PrId(42)).is_none());
}

#[tokio::test]
async fn an_answer_without_json_or_with_a_wrong_document_is_said_so() {
    for (answer, wanted) in [
        ("I looked and all is well.", "no JSON"),
        (r#"{"schema": 2}"#, "cannot be used"),
    ] {
        let dir = TempDir::new("agent-review");
        let _gh = fake_gh().install();
        let mut app = app_asking(&format!("cat >/dev/null; printf '%s' '{answer}'"), &dir);
        app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
        settle(&mut app).await;
        let error = app
            .state
            .store
            .errors
            .get(&PrId(42))
            .cloned()
            .unwrap_or_default();
        assert!(error.contains(wanted), "{answer}: {error}");
    }
}

#[tokio::test]
async fn asking_twice_runs_one_review() {
    let dir = TempDir::new("agent-review");
    let _gh = fake_gh().install();
    let mut app = app_asking("cat >/dev/null; echo '{\"comments\": []}'", &dir);
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    assert_eq!(app.state.store.fetches.len(), 1);
    settle(&mut app).await;
    assert!(
        notice(&app).contains("found nothing to propose"),
        "{}",
        notice(&app)
    );
}

fn with_issues(app: &mut App, issues: Vec<crate::domain::pr::LinkedIssue>) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.info = LoadState::Loaded(crate::domain::pr::PrInfo {
            description: Some("Make it fast.".into()),
            labels: vec![],
            issues,
        });
    }
}

fn issue(number: u64, url: &str) -> crate::domain::pr::LinkedIssue {
    crate::domain::pr::LinkedIssue {
        number,
        title: format!("Issue {number}"),
        url: Some(url.into()),
    }
}

/// An agent that keeps what it is given, and finds nothing.
fn keeping(prompt: &std::path::Path) -> String {
    format!("cat > '{}'; echo '{{\"comments\": []}}'", prompt.display())
}

#[tokio::test]
async fn the_agent_is_given_the_issues_the_rules_and_the_readers_own_instructions() {
    let dir = TempDir::new("agent-review");
    let prompt = dir.path().join("prompt.txt");
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("AGENTS.md"), "Never unwrap.").unwrap();
    let instructions = dir.path().join("review.md");
    std::fs::write(&instructions, "Only look for security problems.").unwrap();

    let _gh = fake_gh()
        .on(
            "issues/12",
            r#"{"title": "Issue 12", "body": "It must be fast."}"#,
        )
        .install();
    let mut app = app_asking(&keeping(&prompt), &dir);
    app.review_root = Some(root);
    app.state.store.agent_review_instructions = Some(instructions);
    with_issues(
        &mut app,
        vec![
            issue(12, "https://github.com/octo/repo/issues/12"),
            // In another repository: its number is not this one's to ask for.
            issue(7, "https://github.com/other/repo/issues/7"),
        ],
    );
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    settle(&mut app).await;

    let given = std::fs::read_to_string(prompt).unwrap();
    for wanted in [
        "Only look for security problems.",
        "--- ISSUE #12",
        "It must be fast.",
        "THE REPOSITORY'S OWN RULES: AGENTS.md",
        "Never unwrap.",
        "Make it fast.",
    ] {
        assert!(given.contains(wanted), "the agent was not given {wanted:?}");
    }
    assert!(
        !given.contains("1. Spec."),
        "the readers instructions replace the built-in"
    );
    assert!(
        !given.contains("ISSUE #7"),
        "an issue of another repository is not read"
    );
    assert!(app.state.store.errors.is_empty());
    assert!(
        !notice(&app).contains("could not be read"),
        "{}",
        notice(&app)
    );
}

#[tokio::test]
async fn without_instructions_or_rules_the_built_in_review_is_enough() {
    let dir = TempDir::new("agent-review");
    let prompt = dir.path().join("prompt.txt");
    let _gh = fake_gh().install();
    let mut app = app_asking(&keeping(&prompt), &dir);
    app.review_root = Some(dir.path().join("a-repo-with-no-rules-files"));
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    settle(&mut app).await;
    let given = std::fs::read_to_string(prompt).unwrap();
    assert!(given.contains("1. Spec.") && given.contains("2. Standards and correctness."));
    assert!(!given.contains("OWN RULES"));
}

#[tokio::test]
async fn instructions_that_cannot_be_read_stop_the_review_and_are_not_replaced_by_the_built_in() {
    let dir = TempDir::new("agent-review");
    let prompt = dir.path().join("prompt.txt");
    let _gh = fake_gh().install();
    let mut app = app_asking(&keeping(&prompt), &dir);
    app.state.store.agent_review_instructions = Some(dir.path().join("missing.md"));
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    settle(&mut app).await;
    let error = app
        .state
        .store
        .errors
        .get(&PrId(42))
        .cloned()
        .unwrap_or_default();
    assert!(
        error.contains("The instructions in") && error.contains("could not be read"),
        "{error}"
    );
    assert!(!prompt.exists(), "the agent was not run");
}

#[tokio::test]
async fn an_issue_that_cannot_be_read_is_said_so_and_the_review_goes_on() {
    let dir = TempDir::new("agent-review");
    let prompt = dir.path().join("prompt.txt");
    let _gh = fake_gh().fail("issues/12", 1, "gh: HTTP 500").install();
    let mut app = app_asking(&keeping(&prompt), &dir);
    with_issues(
        &mut app,
        vec![issue(12, "https://github.com/octo/repo/issues/12")],
    );
    app.apply(Action::Effect(Effect::RunAgentReview { pr_id: PrId(42) }));
    settle(&mut app).await;
    assert!(
        notice(&app).contains("1 issue(s) could not be read"),
        "{}",
        notice(&app)
    );
    assert!(prompt.exists(), "the agent was run without it");
}
