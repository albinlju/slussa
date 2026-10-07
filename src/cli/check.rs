//! What a document an agent hands in is checked against before it is kept: the
//! PR as it is now. A `head` written short is written out when it is the PR's
//! head, and the comments of a document of that head are checked to be on lines
//! of the PR's diff. One that is not could never be shown, so the document is
//! refused and the lines are named, for the agent to correct.

use super::exit::{Failure, Kind};
use crate::{
    domain::{
        commit::CommitOid,
        diff::Diff,
        pr::PrId,
        proposal::{Proposal, Side},
        proposal_document::Parsed,
    },
    providers::{FetchError, Provider, parse_unified_diff},
};

/// How many comments that are not on the diff an error names; the rest are counted.
const NAMED: usize = 10;

/// The comments that are not on a line of `diff`, named for the agent that
/// wrote them: where in the document, and which line it said.
fn off_the_diff(diff: &Diff, head: &CommitOid, comments: &[Proposal]) -> Result<(), Failure> {
    let off: Vec<String> = comments
        .iter()
        .enumerate()
        .filter(|(_, comment)| {
            !diff.has_line(comment.path(), comment.line(), comment.side() == Side::Old)
        })
        .map(|(at, comment)| {
            let side = match comment.side() {
                Side::New => "new",
                Side::Old => "old",
            };
            format!(
                "comments[{at}]: {} line {} ({side})",
                comment.path(),
                comment.line()
            )
        })
        .collect();
    if off.is_empty() {
        return Ok(());
    }
    let more = off.len().saturating_sub(NAMED);
    let mut named = off.into_iter().take(NAMED).collect::<Vec<_>>().join("; ");
    if more > 0 {
        named = format!("{named}; and {more} more");
    }
    Err(Failure::new(
        Kind::Invalid,
        format!(
            "not on a line of the diff of {}, so nothing was kept: {named}. A comment goes on a \
             line the diff adds or leaves unchanged (side new) or removes (side old).",
            head.short()
        ),
    ))
}

/// What became of checking the comments against the PR's diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Lines {
    /// They are on the diff of the PR's head.
    Checked,
    /// There was no diff of the document's commit to check against: it is of
    /// another commit, or the provider does not give the diff as text.
    Unchecked,
    /// The PR moved while this ran: the diff was read at this commit, which is
    /// the PR's head now and not what the document is of.
    Moved(CommitOid),
}

/// Check the comments against the PR's diff, when the document is of the PR's
/// head: that is the diff there is to read.
fn check_lines(
    provider: &Provider,
    pr: PrId,
    parsed: &Parsed,
    current: Option<&CommitOid>,
) -> Result<Lines, Failure> {
    if current != Some(&parsed.head) {
        return Ok(Lines::Unchecked);
    }
    if parsed.comments.is_empty() {
        return Ok(Lines::Checked);
    }
    let (text, revision) = match provider.fetch_diff_text(pr) {
        Ok(read) => read,
        // A provider whose diff is not read as text has nothing to check against.
        Err(FetchError::Unsupported(_)) => return Ok(Lines::Unchecked),
        Err(error) => {
            return Err(Failure::new(
                Kind::Failed,
                format!(
                    "the diff of PR #{pr} could not be read, so the lines could not be checked \
                     and nothing was kept: {}",
                    error.user_message()
                ),
            ));
        }
    };
    // The PR moved while this ran: the diff is of another commit than the document.
    match CommitOid::parse(&revision.head) {
        Some(read) if read != parsed.head => return Ok(Lines::Moved(read)),
        Some(_) => {}
        None => return Ok(Lines::Unchecked),
    }
    off_the_diff(&parse_unified_diff(&text), &parsed.head, &parsed.comments)?;
    Ok(Lines::Checked)
}

/// The document as it is kept: its `head` written out when it is the PR's head
/// written short, and its comments checked against the diff when it is of it.
pub(super) fn checked(
    provider: &Provider,
    pr: PrId,
    parsed: Parsed,
    current_head: Option<&str>,
) -> Result<(Parsed, Lines), Failure> {
    let current = current_head.and_then(CommitOid::parse);
    let parsed = match &current {
        Some(current) => parsed.written_out(current),
        None => parsed,
    };
    let lines = check_lines(provider, pr, &parsed, current.as_ref())?;
    Ok((parsed, lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::proposal_document::{self, Source};

    const HEAD: &str = "abc1234def5678900000000000000000000000ff";

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\n\
--- a/src/a.rs\n\
+++ b/src/a.rs\n\
@@ -1,3 +1,3 @@\n \
keep\n\
-old\n\
+new\n \
last\n";

    /// A document with one comment on each of these lines of `src/a.rs`.
    fn document_at(head: &str, lines: &[(usize, &str)]) -> Parsed {
        let comments: Vec<String> = lines
            .iter()
            .map(|(line, side)| {
                format!(
                    r#"{{"path": "src/a.rs", "line": {line}, "side": "{side}", "body": "Look."}}"#
                )
            })
            .collect();
        let text = format!(
            r#"{{"schema": 1, "head": "{head}", "comments": [{}]}}"#,
            comments.join(",")
        );
        proposal_document::parse(text.as_bytes(), Source::Caller).expect("a document")
    }

    /// GitHub with the PR at `HEAD` and `DIFF` as its diff.
    fn github() -> crate::test_support::InstalledGh {
        crate::test_support::FakeGh::new()
            .on(
                "pulls/44",
                &format!(r#"{{"head": {{"sha": "{HEAD}"}}, "base": {{"sha": "0ba5e00"}}}}"#),
            )
            .on("pr diff", DIFF)
            .install()
    }

    #[test]
    fn a_document_of_the_prs_head_is_checked_against_its_diff() {
        let _gh = github();
        let provider = Provider::github_for_test();
        // An added line, an unchanged one and a removed one are all lines of the diff.
        let on_it = document_at(HEAD, &[(2, "new"), (1, "new"), (2, "old")]);
        let (kept, lines) = checked(&provider, PrId(44), on_it, Some(HEAD))
            .ok()
            .expect("kept");
        assert_eq!(lines, Lines::Checked);
        assert_eq!(kept.comments.len(), 3);
    }

    #[test]
    fn a_comment_off_the_diff_refuses_the_document_and_is_named() {
        let _gh = github();
        let provider = Provider::github_for_test();
        let off = document_at(HEAD, &[(2, "new"), (40, "new"), (3, "old")]);
        let failure = checked(&provider, PrId(44), off, Some(HEAD)).expect_err("refused");
        assert_eq!(failure.kind, Kind::Invalid);
        for wanted in [
            "comments[1]: src/a.rs line 40 (new)",
            "comments[2]: src/a.rs line 3 (old)",
            "nothing was kept",
        ] {
            assert!(failure.message.contains(wanted), "{}", failure.message);
        }
        assert!(
            !failure.message.contains("comments[0]"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn a_head_written_short_is_the_prs_head_and_is_checked_as_it() {
        let _gh = github();
        let provider = Provider::github_for_test();
        let short = document_at("abc1234", &[(2, "new")]);
        let (kept, lines) = checked(&provider, PrId(44), short, Some(HEAD))
            .ok()
            .expect("kept");
        assert_eq!(
            kept.head.as_str(),
            HEAD,
            "written out, so it is shown on the diff"
        );
        assert_eq!(kept.comments[0].head().as_str(), HEAD);
        assert_eq!(lines, Lines::Checked);
    }

    #[test]
    fn a_document_of_another_commit_is_kept_unchecked_and_nothing_is_read() {
        let gh = crate::test_support::FakeGh::new().install();
        let provider = Provider::github_for_test();
        let older = document_at("fff0000aaa", &[(40, "new")]);
        let (kept, lines) = checked(&provider, PrId(44), older, Some(HEAD))
            .ok()
            .expect("kept");
        let calls = gh.calls();
        drop(gh);
        assert_eq!(
            lines,
            Lines::Unchecked,
            "an older commit's diff is not there to read"
        );
        assert_eq!(kept.head.as_str(), "fff0000aaa");
        assert!(calls.is_empty(), "{calls:?}");
    }

    #[test]
    fn a_pr_that_moved_while_the_diff_was_read_is_said_to_have_moved() {
        let _gh = crate::test_support::FakeGh::new()
            .on(
                "pulls/44",
                r#"{"head": {"sha": "9999999aaaa"}, "base": {"sha": "0ba5e00"}}"#,
            )
            .on("pr diff", DIFF)
            .install();
        let provider = Provider::github_for_test();
        let (_, lines) = checked(
            &provider,
            PrId(44),
            document_at(HEAD, &[(2, "new")]),
            Some(HEAD),
        )
        .ok()
        .expect("kept");
        assert_eq!(
            lines,
            Lines::Moved(CommitOid::parse("9999999aaaa").unwrap())
        );
    }

    #[test]
    fn a_diff_that_cannot_be_read_keeps_nothing_and_says_why() {
        let _gh = crate::test_support::FakeGh::new()
            .fail("pulls/44", 1, "gh: HTTP 502: Bad Gateway")
            .install();
        let provider = Provider::github_for_test();
        let failure = checked(
            &provider,
            PrId(44),
            document_at(HEAD, &[(2, "new")]),
            Some(HEAD),
        )
        .expect_err("a failure");
        assert_eq!(failure.kind, Kind::Failed);
        assert!(
            failure.message.contains("could not be checked"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn more_comments_off_the_diff_than_are_named_are_counted() {
        let _gh = github();
        let provider = Provider::github_for_test();
        let lines: Vec<(usize, &str)> = (100..100 + NAMED + 3).map(|line| (line, "new")).collect();
        let failure = checked(&provider, PrId(44), document_at(HEAD, &lines), Some(HEAD))
            .expect_err("refused");
        assert!(
            failure.message.contains("and 3 more"),
            "{}",
            failure.message
        );
    }
}
