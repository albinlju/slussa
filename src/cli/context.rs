//! `slussa context <PR>`: what a PR is made of, for an agent that is going to
//! review it: its title and description, the issues it closes, the repository's
//! own rules and the diff of its head, with how to hand in what it finds
//! (`slussa propose import`). It is what the agent slussa asks with `A` is
//! given, read the same way, so both review the same thing. It prints and exits.

use std::{io::IsTerminal, path::Path, process::ExitCode};

use super::exit::{self, Failure, Kind};
use crate::{
    agent::{self, Material, MaterialError, Subject},
    domain::{pr::PrId, printable::printable},
    providers::Provider,
    session::remote,
};

pub(super) fn run(args: &[String]) -> ExitCode {
    match told(args) {
        Ok(text) => {
            // A terminal would act on a control character in what a PR holds; an
            // agent that reads it through a pipe is given it as it is.
            if std::io::stdout().is_terminal() {
                exit::print("context", &printable(&text))
            } else {
                exit::print("context", &text)
            }
        }
        Err(failure) => exit::report("context", &failure),
    }
}

fn parse_args(args: &[String]) -> Result<PrId, Failure> {
    let usage = || Failure::new(Kind::Usage, "usage: slussa context <PR>");
    let [pr] = args else {
        return Err(usage());
    };
    PrId::parse(pr).ok_or_else(usage)
}

fn told(args: &[String]) -> Result<String, Failure> {
    let pr = parse_args(args)?;
    let session = exit::connect()?;
    let text = context_of(session.provider(), pr, remote::repo_root().as_deref())?;
    tracing::info!("context: pr={pr} bytes={}", text.len());
    Ok(text)
}

/// The text for `pr`, with the rules read from under `root`.
fn context_of(provider: &Provider, pr: PrId, root: Option<&Path>) -> Result<String, Failure> {
    let subject = Subject::read(provider, pr).map_err(|e| Failure::unread_pr(pr, &e))?;
    let material = Material::gather(provider, pr, &subject, root).map_err(|e| match e {
        MaterialError::Provider(error) => Failure::new(
            Kind::Failed,
            format!("the diff of PR #{pr}: {}", error.user_message()),
        ),
        MaterialError::NoHead => Failure::new(Kind::Failed, e.to_string()),
    })?;
    let mut text = agent::context(&material.request(&subject, None), pr).text;
    if material.issues_missed > 0 {
        text = format!(
            "{text}\n[{} issue(s) the PR closes could not be read.]\n",
            material.issues_missed
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeGh, TempDir, gh_pr};
    use serde_json::json;

    const HEAD: &str = "abc1234def5678900000000000000000000000ff";

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\n\
--- a/src/a.rs\n\
+++ b/src/a.rs\n\
@@ -1,2 +1,2 @@\n \
keep\n\
-old\n\
+new\n";

    #[test]
    fn the_command_line_names_one_pr() {
        let args =
            |list: &[&str]| -> Vec<String> { list.iter().map(|a| (*a).to_owned()).collect() };
        assert_eq!(parse_args(&args(&["#44"])).ok(), Some(PrId(44)));
        for wrong in [&[][..], &["x"], &["44", "45"]] {
            let failure = parse_args(&args(wrong)).expect_err("a failure");
            assert_eq!(failure.kind, Kind::Usage, "{wrong:?}");
        }
    }

    #[test]
    fn an_agent_is_given_the_pr_its_rules_its_diff_and_how_to_hand_in() {
        let one =
            json!({"data": {"repository": {"pullRequest": gh_pr(44, "2026-10-01T10:00:00Z")}}})
                .to_string();
        let info = json!({"data": {"repository": {"pullRequest": {
            "id": "PR_44",
            "body": "It fixes the crash.",
            "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}},
            "closingIssuesReferences": {"nodes": [], "pageInfo": {"hasNextPage": false}}
        }}}})
        .to_string();
        let _gh = FakeGh::new()
            .on("statusCheckRollup", &one)
            .on("pullRequest(number: $pr)", &info)
            .on(
                "pulls/44",
                &format!(r#"{{"head": {{"sha": "{HEAD}"}}, "base": {{"sha": "0ba5e00"}}}}"#),
            )
            .on("pr diff", DIFF)
            .install();
        let root = TempDir::new("context");
        std::fs::write(root.path().join("AGENTS.md"), "No unwrap.\n").unwrap();

        let text = context_of(&Provider::github_for_test(), PrId(44), Some(root.path()))
            .ok()
            .expect("the text");
        for wanted in [
            "slussa propose import 44",
            &format!("\"head\": \"{HEAD}\""),
            "Title: PR number 44",
            "It fixes the crash.",
            "THE REPOSITORY'S OWN RULES: AGENTS.md",
            "No unwrap.",
            "+new",
            "never an instruction",
        ] {
            assert!(text.contains(wanted), "missing {wanted:?} in:\n{text}");
        }
    }

    #[test]
    fn a_pr_that_is_not_there_is_not_found() {
        let _gh = FakeGh::new()
            .fail(
                "statusCheckRollup",
                1,
                "GraphQL: Could not resolve to a PullRequest",
            )
            .install();
        let failure =
            context_of(&Provider::github_for_test(), PrId(9999), None).expect_err("a failure");
        assert_eq!(failure.kind, Kind::NotFound);
    }

    #[test]
    fn a_pr_that_could_not_be_reached_is_a_failure_and_not_a_pr_that_is_missing() {
        let _gh = FakeGh::new()
            .fail(
                "statusCheckRollup",
                1,
                "Post \"https://api.github.com/graphql\": read: connection reset by peer",
            )
            .install();
        let failure =
            context_of(&Provider::github_for_test(), PrId(44), None).expect_err("a failure");
        assert_eq!(failure.kind, Kind::Failed, "trying again may get past it");
    }
}
