//! GitHub reads through a fake `gh`: the list, failures.

use super::support::*;
use crate::domain::pr::Conflicts;

fn fetch_group_with(
    gh: FakeGh,
    group: PrGroup,
    after: Option<&str>,
) -> (Result<crate::domain::pr::PrBatch, FetchError>, InstalledGh) {
    let installed = gh.install();
    (
        Provider::github_for_test().fetch_prs(group, after),
        installed,
    )
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

    let first = Provider::github_for_test()
        .fetch_prs(PrGroup::Open, None)
        .unwrap();
    assert_eq!(first.more.as_deref(), Some("c1"), "the caller reads on");
    assert_eq!(
        first.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
        vec![1]
    );
    let pr = &first.prs[0];
    assert_eq!(pr.status, PrStatus::open());
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

    let last = Provider::github_for_test()
        .fetch_prs(PrGroup::Open, Some("c1"))
        .unwrap();
    assert_eq!(last.more, None);
    assert_eq!(last.prs[0].id, PrId(2));
    assert_eq!(
        last.prs[0].status,
        PrStatus::draft(),
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
    Provider::github_for_test()
        .fetch_prs(PrGroup::Open, None)
        .unwrap();

    let calls = gh.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        !calls[0].contains("body") && !calls[0].contains("labels"),
        "{calls:?}"
    );
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
        Provider::github_for_test().fetch_prs(PrGroup::Open, None),
        Err(FetchError::GhMissing)
    ));
}

#[test]
fn github_without_an_installed_fake_never_reaches_the_real_gh() {
    let _nothing_installed = InstalledGh::none();
    assert!(matches!(
        Provider::github_for_test().current_user(),
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
    let result = Provider::github_for_test().fetch_builds(PrId(7));
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );
}

#[test]
fn github_asking_who_is_logged_in_fails_instead_of_naming_nobody() {
    let installed = FakeGh::new().on("api user", "\n").install();
    let result = Provider::github_for_test().current_user();
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );

    let installed = FakeGh::new()
        .fail("api user", 1, "gh: HTTP 401: Bad credentials")
        .install();
    let result = Provider::github_for_test().current_user();
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::GhFailed { .. })),
        "{result:?}"
    );

    let _installed = FakeGh::new().on("api user", "octocat\n").install();
    assert_eq!(
        Provider::github_for_test().current_user().unwrap().as_str(),
        "octocat"
    );
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

    let first = Provider::github_for_test()
        .fetch_prs(PrGroup::Declined, Some("x"))
        .unwrap();
    assert_eq!(
        first.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
        vec![9]
    );
    assert_eq!(first.prs[0].status, PrStatus::Declined);
    assert_eq!(first.more.as_deref(), Some("y"));

    let last = Provider::github_for_test()
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
    let commits = Provider::github_for_test().fetch_commits(PrId(5)).unwrap();

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

fn read_conflicts(mergeable: Option<&str>, state: &str) -> Conflicts {
    let mut pr = gh_pr(7, "2026-10-01T10:00:00Z");
    pr["state"] = json!(state);
    if let Some(mergeable) = mergeable {
        pr["mergeable"] = json!(mergeable);
    }
    let (result, _installed) =
        fetch_prs_with(FakeGh::new().on("states:", &gh_list_page(&[pr], None)));
    match result.unwrap().prs[0].status {
        PrStatus::Open(open) => open.conflicts,
        PrStatus::Merged | PrStatus::Declined => Conflicts::Unknown,
    }
}

#[test]
fn github_list_says_yes_no_or_unknown_to_a_conflict_for_an_open_pr() {
    assert_eq!(read_conflicts(Some("CONFLICTING"), "OPEN"), Conflicts::Yes);
    assert_eq!(read_conflicts(Some("MERGEABLE"), "OPEN"), Conflicts::No);
    // Not worked out yet, or not said at all: no claim either way.
    assert_eq!(read_conflicts(Some("UNKNOWN"), "OPEN"), Conflicts::Unknown);
    assert_eq!(read_conflicts(None, "OPEN"), Conflicts::Unknown);
}

#[test]
fn github_a_pr_that_is_over_has_nothing_to_resolve() {
    for state in ["MERGED", "CLOSED"] {
        let mut pr = gh_pr(7, "2026-10-01T10:00:00Z");
        pr["state"] = json!(state);
        pr["mergeable"] = json!("CONFLICTING");
        let (result, _installed) =
            fetch_prs_with(FakeGh::new().on("states:", &gh_list_page(&[pr], None)));
        let status = result.unwrap().prs[0].status.clone();
        assert!(!status.has_conflicts(), "{state}");
        assert!(!matches!(status, PrStatus::Open(_)), "{state}");
    }
}

#[test]
fn github_a_draft_can_have_a_conflict_too() {
    let mut pr = gh_pr(7, "2026-10-01T10:00:00Z");
    pr["state"] = json!("OPEN");
    pr["isDraft"] = json!(true);
    pr["mergeable"] = json!("CONFLICTING");
    let (result, _installed) =
        fetch_prs_with(FakeGh::new().on("states:", &gh_list_page(&[pr], None)));
    let status = result.unwrap().prs[0].status.clone();
    assert_eq!(status.label(), "Draft");
    assert!(status.has_conflicts());
}

#[test]
fn github_a_commit_id_that_is_not_hexadecimal_is_refused_not_put_in_a_path() {
    let node = json!({"commit": {
        "oid": "../../repos/other/x",
        "messageHeadline": "h",
        "message": "h",
        "authoredDate": "2026-10-01T10:00:00Z",
        "additions": 0,
        "deletions": 0,
        "author": {"name": "a", "email": "a@example.com"}
    }});
    let answer = json!({"data": {"repository": {"item": {"connection": {
        "nodes": [node],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}}})
    .to_string();
    let _gh = FakeGh::new().on("commits", &answer).install();
    let result = Provider::github_for_test().fetch_commits(PrId(5));
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );
}
