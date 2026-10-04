//! GitHub reads through a fake `gh`: the list, one PR's details, failures.

use super::support::*;

fn fetch_group_with(
    gh: FakeGh,
    group: PrGroup,
    after: Option<&str>,
) -> (Result<crate::domain::pr::PrBatch, FetchError>, InstalledGh) {
    let installed = gh.install();
    (Provider::GitHub.fetch_prs(group, after), installed)
}

fn fetch_prs_with(gh: FakeGh) -> (Result<crate::domain::pr::PrBatch, FetchError>, InstalledGh) {
    fetch_group_with(gh, PrGroup::Open, None)
}

#[test]
fn github_open_group_is_read_a_page_at_a_time_with_drafts_and_nothing_closed() {
    let mut draft = gh_pr(2, "2026-09-03T10:00:00Z");
    draft["isDraft"] = json!(true);
    let gh = FakeGh::new()
        .on(
            "states: OPEN, first: 30, after: null",
            &gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], Some("c1")),
        )
        .on(
            "states: OPEN, first: 30, after: \"c1\"",
            &gh_list_page(&[draft], None),
        )
        .install();

    let first = Provider::GitHub.fetch_prs(PrGroup::Open, None).unwrap();
    assert_eq!(first.more.as_deref(), Some("c1"), "the caller reads on");
    assert_eq!(
        first.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
        vec![1]
    );
    let pr = &first.prs[0];
    assert_eq!(pr.status, PrStatus::Open);
    assert_eq!(pr.author.username, "alice");
    assert!(
        pr.labels.is_empty() && pr.description.is_none(),
        "the list leaves out the description and the labels"
    );
    assert_eq!(
        pr.comment_count, 6,
        "four conversation comments and two threads on code"
    );
    assert_eq!(pr.reviewers.len(), 1);

    let last = Provider::GitHub
        .fetch_prs(PrGroup::Open, Some("c1"))
        .unwrap();
    assert_eq!(last.more, None);
    assert_eq!(last.prs[0].id, PrId(2));
    assert_eq!(
        last.prs[0].status,
        PrStatus::Draft,
        "drafts come with the open ones"
    );

    let calls = gh.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].starts_with("api graphql -F owner={owner} -F name={repo} -f query="));
    assert!(calls[0].contains("pullRequests(states: OPEN, first: 30, after: null)"));
    assert!(calls[1].contains("pullRequests(states: OPEN, first: 30, after: \"c1\")"));
    assert!(
        calls
            .iter()
            .all(|call| !call.contains("MERGED") && !call.contains("CLOSED")),
        "no closed PRs are asked for: {calls:?}"
    );
}

#[test]
fn github_closed_groups_read_one_page_each_with_their_own_state() {
    for (group, state, args) in [
        (
            PrGroup::Merged,
            "MERGED",
            "states: MERGED, orderBy: {field: UPDATED_AT, direction: DESC}, first: 30, after: null",
        ),
        (
            PrGroup::Declined,
            "CLOSED",
            "states: CLOSED, orderBy: {field: UPDATED_AT, direction: DESC}, first: 30, after: null",
        ),
    ] {
        // More exist, but reading the group must not follow that cursor.
        let gh = FakeGh::new().on(
            "after: null",
            &gh_list_page(&[gh_closed_pr(3, "2026-08-01T10:00:00Z", state)], Some("x")),
        );
        let (result, installed) = fetch_group_with(gh, group, None);
        let batch = result.unwrap();

        assert_eq!(batch.prs.len(), 1);
        assert_eq!(PrGroup::of(&batch.prs[0].status), group, "{group:?}");
        assert_eq!(batch.more.as_deref(), Some("x"));
        let calls = installed.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].contains(args), "{group:?}: {}", calls[0]);
    }
}

#[test]
fn github_a_closed_pr_still_flagged_as_a_draft_is_not_shown_as_one() {
    // GitHub keeps the draft flag on a closed PR. Seen on a real repository.
    for (state, expected) in [("CLOSED", PrStatus::Declined), ("MERGED", PrStatus::Merged)] {
        let mut pr = gh_closed_pr(3, "2026-08-01T10:00:00Z", state);
        pr["isDraft"] = json!(true);
        let gh = FakeGh::new().on("states:", &gh_list_page(&[pr], None));
        let group = PrGroup::of(&expected);
        let (result, _installed) = fetch_group_with(gh, group, None);

        assert_eq!(result.unwrap().prs[0].status, expected, "{state}");
    }
}

#[test]
fn github_a_failed_read_is_an_error_for_that_group() {
    for group in PrGroup::ALL {
        let gh = FakeGh::new().fail("states:", 1, "gh: HTTP 502");
        let (result, installed) = fetch_group_with(gh, group, None);

        assert!(
            matches!(result, Err(FetchError::GhFailed { .. })),
            "{group:?}: {result:?}"
        );
        assert_eq!(installed.calls().len(), 1);
    }
}

#[test]
fn github_review_requests_become_pending_reviewers_and_replace_an_earlier_review() {
    let mut node = gh_pr(1, "2026-09-01T10:00:00Z");
    node["latestReviews"]["nodes"] = json!([
        {"state": "APPROVED", "author": {"login": "me"}},
        {"state": "APPROVED", "author": {"login": "bob"}}
    ]);
    // `me` is asked again; the team request and the null have no login.
    node["reviewRequests"] = json!({"nodes": [
        {"requestedReviewer": {"login": "me"}},
        {"requestedReviewer": {}},
        {"requestedReviewer": null}
    ]});
    let (result, installed) = fetch_prs_with(
        FakeGh::new()
            .on("states: OPEN", &gh_list_page(&[node], None))
            .on("states: [MERGED, CLOSED]", &gh_list_page(&[], None)),
    );

    let reviewers = &result.unwrap().prs[0].reviewers;
    let summary: Vec<_> = reviewers
        .iter()
        .map(|r| (r.author.username.as_str(), r.state.clone()))
        .collect();
    assert_eq!(
        summary,
        [
            ("bob", ReviewerState::Approved),
            ("me", ReviewerState::Requested)
        ]
    );
    assert!(installed.calls()[0].contains("reviewRequests(first: 100)"));
}

#[test]
fn github_list_query_leaves_out_the_body_and_the_labels() {
    let gh = FakeGh::new()
        .on(
            "states: OPEN",
            &gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], None),
        )
        .install();
    Provider::GitHub.fetch_prs(PrGroup::Open, None).unwrap();

    let calls = gh.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        !calls[0].contains("body") && !calls[0].contains("labels"),
        "{calls:?}"
    );
}

fn info_answer(labels: &[&str], more: bool) -> String {
    json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Explains the change.",
        "labels": {
            "nodes": labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
            "pageInfo": {"hasNextPage": more}
        }
    }}}})
    .to_string()
}

#[test]
fn github_info_reads_the_description_and_the_labels_of_one_pr() {
    let gh = FakeGh::new()
        .on("pullRequest(number: $pr)", &info_answer(&["bug"], false))
        .install();
    let info = Provider::GitHub.fetch_info(PrId(7)).unwrap();

    assert_eq!(info.description.as_deref(), Some("Explains the change."));
    assert_eq!(info.labels, vec!["bug"]);
    assert_eq!(gh.calls().len(), 1);
    assert!(gh.calls()[0].contains("pr=7"));
}

#[test]
fn github_info_reads_on_when_the_labels_are_truncated() {
    let node_page = json!({"data": {"item": {"connection": {
        "nodes": [{"name": "bug"}, {"name": "ux"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}})
    .to_string();
    let gh = FakeGh::new()
        .on("node(id: \"PR_1\")", &node_page)
        .on("pullRequest(number: $pr)", &info_answer(&["bug"], true))
        .install();
    let info = Provider::GitHub.fetch_info(PrId(1)).unwrap();

    assert_eq!(info.labels, vec!["bug", "ux"]);
    assert_eq!(gh.calls().len(), 2);
}

#[test]
fn only_github_reads_the_description_and_labels_apart_from_the_list() {
    use crate::domain::capabilities::Feature;
    assert!(Provider::GitHub.capabilities().supports(Feature::PrInfo));
}

#[test]
fn github_failure_carries_the_exit_code_and_stderr() {
    let gh = FakeGh::new().fail("graphql", 1, "gh: HTTP 401: Bad credentials");
    let (result, _installed) = fetch_prs_with(gh);

    match result.unwrap_err() {
        FetchError::GhFailed { code, stderr, .. } => {
            assert_eq!(code, Some(1));
            assert!(stderr.contains("Bad credentials"), "{stderr}");
        }
        other => panic!("expected GhFailed, got {other:?}"),
    }
}

#[test]
fn github_reports_a_missing_gh() {
    let _installed = InstalledGh::missing();
    assert!(matches!(
        Provider::GitHub.fetch_prs(PrGroup::Open, None),
        Err(FetchError::GhMissing)
    ));
}

#[test]
fn github_without_an_installed_fake_never_reaches_the_real_gh() {
    let _nothing_installed = InstalledGh::none();
    assert!(matches!(
        Provider::GitHub.current_user(),
        Err(FetchError::GhMissing)
    ));
}

#[test]
fn github_rejects_unparseable_output_and_graphql_errors() {
    // One fake at a time: the first guard must be gone before the second install.
    let (result, installed) = fetch_prs_with(FakeGh::new().on("graphql", "not json"));
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );

    let errors = json!({"errors": [{"message": "rate limited"}]}).to_string();
    let (result, _installed) = fetch_prs_with(FakeGh::new().on("graphql", &errors));
    // GitHub's own words are kept for the log; the user gets one plain line.
    let error = result.unwrap_err();
    assert!(
        matches!(&error, FetchError::GraphQl(messages) if messages == &["rate limited"]),
        "{error:?}"
    );
    assert_eq!(
        error.user_message(),
        "GitHub returned an incomplete GraphQL response."
    );
}

#[test]
fn github_an_answer_of_the_wrong_shape_is_an_error_and_not_a_panic() {
    // The connection is a list where an object with `nodes` and `pageInfo` belongs.
    let wrong = json!({"data": {"repository": {"connection": [1, 2]}}}).to_string();
    let (result, installed) = fetch_prs_with(FakeGh::new().on("graphql", &wrong));
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );

    // A REST page that is a list where an object with `check_runs` belongs.
    let _installed = FakeGh::new()
        .on("check-runs", "[[1, 2]]")
        .on("pulls/7", "abc\n")
        .install();
    let result = Provider::GitHub.fetch_builds(PrId(7));
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );
}

#[test]
fn github_asking_who_is_logged_in_fails_instead_of_naming_nobody() {
    let installed = FakeGh::new().on("api user", "\n").install();
    let result = Provider::GitHub.current_user();
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );

    let installed = FakeGh::new()
        .fail("api user", 1, "gh: HTTP 401: Bad credentials")
        .install();
    let result = Provider::GitHub.current_user();
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::GhFailed { .. })),
        "{result:?}"
    );

    let _installed = FakeGh::new().on("api user", "octocat\n").install();
    assert_eq!(Provider::GitHub.current_user().unwrap().as_str(), "octocat");
}

#[test]
fn github_older_prs_continue_from_the_cursor_and_report_when_they_end() {
    let gh = FakeGh::new()
        .on(
            "after: \"x\"",
            &gh_list_page(
                &[gh_closed_pr(9, "2026-08-01T10:00:00Z", "CLOSED")],
                Some("y"),
            ),
        )
        .on(
            "after: \"y\"",
            &gh_list_page(&[gh_closed_pr(8, "2026-07-01T10:00:00Z", "CLOSED")], None),
        )
        .install();

    let first = Provider::GitHub
        .fetch_prs(PrGroup::Declined, Some("x"))
        .unwrap();
    assert_eq!(
        first.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
        vec![9]
    );
    assert_eq!(first.prs[0].status, PrStatus::Declined);
    assert_eq!(first.more.as_deref(), Some("y"));

    let last = Provider::GitHub
        .fetch_prs(PrGroup::Declined, Some("y"))
        .unwrap();
    assert_eq!(
        last.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
        vec![8]
    );
    assert_eq!(last.more, None, "the end is reported, not guessed");

    let calls = gh.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    for call in &calls {
        assert!(call.contains("states: CLOSED"), "{call}");
        assert!(call.contains("first: 30"), "{call}");
        assert!(
            !call.contains("states: OPEN"),
            "older reads skip open PRs: {call}"
        );
    }
}

#[test]
fn github_commits_carry_their_message_and_a_bot_address_marks_a_bot() {
    let node = |oid: &str, name: &str, email: &str, message: &str| {
        json!({"commit": {
            "oid": oid,
            "messageHeadline": message.lines().next().unwrap_or_default(),
            "message": message,
            "authoredDate": "2026-10-01T10:00:00Z",
            "additions": 1,
            "deletions": 0,
            "author": {"name": name, "email": email}
        }})
    };
    let answer = json!({"data": {"repository": {"item": {"connection": {
        "nodes": [
            node("aaa1111", "Alice", "alice@example.com",
                 "Fix the lock\n\nCo-Authored-By: Claude <noreply@anthropic.com>"),
            node("bbb2222", "claude[bot]", "1+claude[bot]@users.noreply.github.com", "Review fixes"),
        ],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}}})
    .to_string();
    let _gh = FakeGh::new().on("commits", &answer).install();
    let commits = Provider::GitHub.fetch_commits(PrId(5)).unwrap();

    assert_eq!(commits.len(), 2);
    assert!(commits[0].message.ends_with("<noreply@anthropic.com>"));
    assert_eq!(commits[0].account, crate::domain::user::AccountKind::Person);
    assert_eq!(commits[1].account, crate::domain::user::AccountKind::Bot);
}

fn review_by(typename: &str, state: &str, oid: &str) -> Value {
    json!({"state": state, "author": {"__typename": typename, "login": "reviewer"}, "commit": {"oid": oid}})
}

fn read_ai(reviews: &[Value], head: &str, state: &str) -> crate::domain::pr::AiReview {
    let mut pr = gh_pr(7, "2026-10-01T10:00:00Z");
    pr["state"] = json!(state);
    pr["headRefOid"] = json!(head);
    pr["latestReviews"] = json!({"nodes": reviews, "pageInfo": {"hasNextPage": false}});
    let (result, _installed) =
        fetch_prs_with(FakeGh::new().on("states:", &gh_list_page(&[pr], None)));
    result.unwrap().prs[0].ai_review
}

#[test]
fn github_list_tells_a_bots_review_from_a_persons_and_says_how_it_stands() {
    use crate::domain::pr::AiReview;
    let bot = |state: &str, oid: &str| review_by("Bot", state, oid);
    // Nobody but a person has reviewed.
    assert_eq!(
        read_ai(&[review_by("User", "APPROVED", "head")], "head", "OPEN"),
        AiReview::None
    );
    assert_eq!(read_ai(&[], "head", "OPEN"), AiReview::None);
    // A bot reviewed the head, or an older commit, or asked for changes.
    assert_eq!(
        read_ai(&[bot("COMMENTED", "head")], "head", "OPEN"),
        AiReview::Current
    );
    assert_eq!(
        read_ai(&[bot("COMMENTED", "old")], "head", "OPEN"),
        AiReview::Stale
    );
    assert_eq!(
        read_ai(&[bot("CHANGES_REQUESTED", "head")], "head", "OPEN"),
        AiReview::ChangesRequested
    );
    // With several, the one that needs the reader most counts.
    assert_eq!(
        read_ai(
            &[bot("COMMENTED", "head"), bot("COMMENTED", "old")],
            "head",
            "OPEN"
        ),
        AiReview::Stale
    );
    assert_eq!(
        read_ai(
            &[bot("APPROVED", "old"), bot("CHANGES_REQUESTED", "old")],
            "head",
            "OPEN"
        ),
        AiReview::ChangesRequested
    );
}

#[test]
fn github_list_calls_a_review_of_a_pr_that_is_over_only_a_review() {
    use crate::domain::pr::AiReview;
    let old = [review_by("Bot", "CHANGES_REQUESTED", "old")];
    for state in ["MERGED", "CLOSED"] {
        assert_eq!(read_ai(&old, "head", state), AiReview::Current, "{state}");
    }
    assert_eq!(read_ai(&[], "head", "MERGED"), AiReview::None);
}

#[test]
fn github_list_does_not_call_a_review_current_when_its_commit_is_gone() {
    use crate::domain::pr::AiReview;
    // GitHub gives null for a commit it no longer has, after a force-push.
    let gone = json!({"state": "COMMENTED", "author": {"__typename": "Bot", "login": "bot"}, "commit": null});
    assert_eq!(read_ai(&[gone], "head", "OPEN"), AiReview::Stale);
}

#[test]
fn github_reads_one_pr_by_its_number_as_the_list_holds_it() {
    let answer =
        json!({"data": {"repository": {"pullRequest": gh_pr(44, "2026-10-01T10:00:00Z")}}})
            .to_string();
    let gh = FakeGh::new().on("statusCheckRollup", &answer).install();
    let pr = Provider::GitHub.fetch_pr(PrId(44)).unwrap();

    assert_eq!((pr.id, pr.title.as_str()), (PrId(44), "PR number 44"));
    assert_eq!(gh.calls().len(), 1);
    assert!(gh.calls()[0].contains("pr=44"), "{:?}", gh.calls());
}

#[test]
fn github_reports_a_pr_that_does_not_exist_as_an_error() {
    let _gh = FakeGh::new()
        .fail(
            "statusCheckRollup",
            1,
            "GraphQL: Could not resolve to a PullRequest",
        )
        .install();
    assert!(Provider::GitHub.fetch_pr(PrId(9999)).is_err());
}

#[test]
fn github_info_reads_the_issues_the_pr_closes() {
    let answer = json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Fixes it.",
        "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}},
        "closingIssuesReferences": {"nodes": [
            {"number": 12, "title": "Crash on start"},
            {"number": 31, "title": "Slow list"}
        ], "pageInfo": {"hasNextPage": false}}
    }}}})
    .to_string();
    let _gh = FakeGh::new()
        .on("pullRequest(number: $pr)", &answer)
        .install();
    let info = Provider::GitHub.fetch_info(PrId(7)).unwrap();

    let issues: Vec<_> = info
        .issues
        .iter()
        .map(|issue| (issue.number, issue.title.as_str()))
        .collect();
    assert_eq!(issues, [(12, "Crash on start"), (31, "Slow list")]);
}

#[test]
fn github_info_reads_on_when_the_closing_issues_are_truncated() {
    let first_page = json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Fixes many.",
        "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}},
        "closingIssuesReferences": {
            "nodes": [{"number": 1, "title": "One"}],
            "pageInfo": {"hasNextPage": true}
        }
    }}}})
    .to_string();
    let all = json!({"data": {"item": {"connection": {
        "nodes": [{"number": 1, "title": "One"}, {"number": 2, "title": "Two"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}})
    .to_string();
    let gh = FakeGh::new()
        .on("node(id: \"PR_1\")", &all)
        .on("pullRequest(number: $pr)", &first_page)
        .install();
    let info = Provider::GitHub.fetch_info(PrId(1)).unwrap();

    let numbers: Vec<_> = info.issues.iter().map(|issue| issue.number).collect();
    assert_eq!(numbers, [1, 2]);
    assert_eq!(gh.calls().len(), 2);
}

#[test]
fn github_a_pr_answered_with_null_is_said_not_to_exist() {
    let answer = json!({"data": {"repository": {"pullRequest": null}}}).to_string();
    let _gh = FakeGh::new().on("statusCheckRollup", &answer).install();
    let error = Provider::GitHub.fetch_pr(PrId(9999)).unwrap_err();
    assert!(error.user_message().contains("no PR #9999"), "{error:?}");
}
