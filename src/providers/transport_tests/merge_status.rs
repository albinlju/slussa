//! Merge status: what stands in the way of merging, on both providers.

use super::support::*;

fn gh_merge_fields(mergeable: &str, state: &str, decision: Option<&str>) -> String {
    json!({"data": {"repository": {"pullRequest": {
        "mergeable": mergeable,
        "mergeStateStatus": state,
        "reviewDecision": decision
    }}}})
    .to_string()
}

/// The same state, whatever reasons it carries.
fn same_state(a: &Mergeability, b: &Mergeability) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

#[test]
fn github_merge_status_reads_the_state_and_explains_a_block() {
    let cases: [(&str, &str, Option<&str>, Mergeability, &str); 9] = [
        (
            "MERGEABLE",
            "CLEAN",
            Some("APPROVED"),
            Mergeability::Mergeable,
            "",
        ),
        ("MERGEABLE", "UNSTABLE", None, Mergeability::Mergeable, ""),
        (
            "CONFLICTING",
            "DIRTY",
            None,
            Mergeability::Conflicts(vec![]),
            "conflicts with the base",
        ),
        (
            "MERGEABLE",
            "BEHIND",
            None,
            Mergeability::Blocked(vec![]),
            "behind its base",
        ),
        (
            "MERGEABLE",
            "DRAFT",
            None,
            Mergeability::Blocked(vec![]),
            "draft",
        ),
        (
            "MERGEABLE",
            "BLOCKED",
            Some("REVIEW_REQUIRED"),
            Mergeability::Blocked(vec![]),
            "approving review",
        ),
        (
            "MERGEABLE",
            "BLOCKED",
            Some("CHANGES_REQUESTED"),
            Mergeability::Blocked(vec![]),
            "requested changes",
        ),
        (
            "MERGEABLE",
            "BLOCKED",
            Some("APPROVED"),
            Mergeability::Blocked(vec![]),
            "Required checks",
        ),
        ("UNKNOWN", "UNKNOWN", None, Mergeability::Unknown, ""),
    ];
    for (mergeable, state, decision, expected, reason) in cases {
        let installed = FakeGh::new()
            .on("graphql", &gh_merge_fields(mergeable, state, decision))
            .install();
        let status = Provider::GitHub.fetch_mergeability(PrId(7)).unwrap();

        assert!(
            same_state(&status, &expected),
            "{mergeable} {state} {decision:?}: {status:?}"
        );
        if reason.is_empty() {
            assert!(status.blockers().is_empty(), "{status:?}");
        } else {
            assert!(
                status.blockers().iter().any(|b| b.contains(reason)),
                "{state} {decision:?}: {status:?}"
            );
        }
        let calls = installed.calls();
        assert!(
            calls[0].contains("mergeStateStatus reviewDecision"),
            "{calls:?}"
        );
        drop(installed);
    }
}

fn merge_url(pr: u64) -> String {
    format!("{PR_BASE}/{pr}/merge")
}

#[test]
fn bitbucket_merge_checks_become_blockers_with_the_servers_words() {
    let blocked = json!({"canMerge": false, "conflicted": false, "vetoes": [
        {"summaryMessage": "Not all required builds are successful yet", "detailedMessage": "x"},
        {"summaryMessage": "At least 1 approval is required", "detailedMessage": "y"}
    ]});
    let server = MockHttp::start(vec![Route::get(&merge_url(9), 200, &blocked.to_string())]);
    let status = bitbucket(&server).fetch_mergeability(PrId(9)).unwrap();

    assert_eq!(
        status,
        Mergeability::Blocked(vec![
            "Not all required builds are successful yet".into(),
            "At least 1 approval is required".into()
        ])
    );
}

#[test]
fn bitbucket_merge_status_covers_conflict_clean_and_a_veto_without_words() {
    let cases = [
        (
            json!({"canMerge": false, "conflicted": true, "vetoes": []}),
            Mergeability::Conflicts(vec![]),
            "merge conflicts",
        ),
        (
            json!({"canMerge": false, "conflicted": false}),
            Mergeability::Blocked(vec![]),
            "Merge checks have not passed",
        ),
        (
            json!({"canMerge": true, "conflicted": false}),
            Mergeability::Mergeable,
            "",
        ),
    ];
    for (body, expected, reason) in cases {
        let server = MockHttp::start(vec![Route::get(&merge_url(9), 200, &body.to_string())]);
        let status = bitbucket(&server).fetch_mergeability(PrId(9)).unwrap();

        assert!(same_state(&status, &expected), "{body}: {status:?}");
        assert_eq!(
            status.blockers().iter().any(|b| b.contains(reason)),
            !reason.is_empty(),
            "{body}: {status:?}"
        );
    }
}

#[test]
fn bitbucket_refused_merge_reports_the_vetoes() {
    let refusal = json!({"errors": [{
        "message": "Merging is vetoed",
        "exceptionName": "PullRequestMergeVetoedException",
        "vetoes": [
            {"summaryMessage": "Not all required builds are successful yet"},
            {"summaryMessage": "At least 1 approval is required"}
        ]
    }]});
    let server = MockHttp::start(vec![
        Route::get(
            &format!("{PR_BASE}/9"),
            200,
            &json!({"version": 3}).to_string(),
        ),
        Route::post(
            &format!("{}?version=3", merge_url(9)),
            409,
            &refusal.to_string(),
        ),
    ]);
    let error = bitbucket(&server)
        .merge(PrId(9), MergeStrategy::Merge)
        .unwrap_err();

    assert_eq!(
        error.user_message(),
        "Merging is vetoed: Not all required builds are successful yet; At least 1 approval is required"
    );
}

#[test]
fn github_a_pr_set_to_merge_by_itself_says_how_and_what_it_waits_for() {
    let answer = |mergeable: &str, state: &str| {
        json!({"data": {"repository": {"pullRequest": {
            "mergeable": mergeable,
            "mergeStateStatus": state,
            "reviewDecision": "REVIEW_REQUIRED",
            "autoMergeRequest": {"mergeMethod": "SQUASH"}
        }}}})
        .to_string()
    };
    let gh = FakeGh::new()
        .on("graphql", &answer("MERGEABLE", "BLOCKED"))
        .install();
    let status = Provider::GitHub.fetch_mergeability(PrId(7)).unwrap();
    let Mergeability::AutoMerge { strategy, waiting } = status else {
        panic!("{status:?}");
    };
    assert_eq!(strategy, MergeStrategy::Squash);
    assert!(
        waiting
            .iter()
            .any(|reason| reason.contains("approving review"))
    );
    drop(gh);

    // A conflict is one more thing it waits for, and the request stays there to
    // be turned off.
    let _second = FakeGh::new()
        .on("graphql", &answer("CONFLICTING", "DIRTY"))
        .install();
    let status = Provider::GitHub.fetch_mergeability(PrId(7)).unwrap();
    let Mergeability::AutoMerge { strategy, waiting } = status else {
        panic!("{status:?}");
    };
    assert_eq!(strategy, MergeStrategy::Squash);
    assert!(waiting.iter().any(|reason| reason.contains("conflicts")));
}
