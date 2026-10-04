//! What a pull request asks of the person looking at the list.
//!
//! Derived from data the list already has: the viewer, the author, the
//! reviewers' states and the CI summary, and what the viewer has looked at, for
//! comments that came after. Mentions are not here yet.

use super::{
    ci::CiSummary,
    pr::{PrStatus, PullRequest},
    review::ReviewerState,
    seen::Seen,
    user::Username,
};

/// Why a PR needs you. Variants are declared from most to least urgent, and
/// the derived order is what the list sorts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Attention {
    /// Your PR: a reviewer asked for changes.
    ChangesRequested,
    /// Your PR: the checks failed.
    CiFailed,
    /// Someone else's PR: you were asked to review it and have not.
    ReviewRequested,
    /// Your PR: every reviewer approved, so the decision is yours.
    Approved,
    /// A PR you have opened has more comments than when you last looked. The
    /// least urgent, so it only stands where nothing above does.
    NewComments,
}

impl Attention {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChangesRequested => "changes requested",
            Self::CiFailed => "CI failed",
            Self::ReviewRequested => "review requested",
            Self::Approved => "approved",
            Self::NewComments => "new comments",
        }
    }
}

/// The reason `pr` needs `viewer`, if any. Only open PRs ask for anything.
/// Usernames compare without regard to case, as providers differ on that.
pub fn attention(pr: &PullRequest, viewer: &Username, seen: &Seen) -> Option<Attention> {
    if pr.status != PrStatus::Open {
        return None;
    }
    of_role(pr, viewer).or_else(|| seen.has_new_comments(pr).then_some(Attention::NewComments))
}

/// What the viewer's part in the PR, as its author or a reviewer, asks of them.
fn of_role(pr: &PullRequest, viewer: &Username) -> Option<Attention> {
    let is = |name: &str| viewer.is(name);
    if is(&pr.author.username) {
        let states = || pr.reviewers.iter().map(|r| &r.state);
        if states().any(|s| *s == ReviewerState::ChangesRequested) {
            Some(Attention::ChangesRequested)
        } else if pr.ci == CiSummary::Failed {
            Some(Attention::CiFailed)
        } else if pr.reviewers.is_empty() || !states().all(|s| *s == ReviewerState::Approved) {
            None
        } else {
            Some(Attention::Approved)
        }
    } else {
        pr.reviewers
            .iter()
            .any(|r| is(&r.author.username) && r.state == ReviewerState::Requested)
            .then_some(Attention::ReviewRequested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{pr::PrId, review::Reviewer, user::User};
    use chrono::Utc;

    /// What the reason is for a reader who has looked at nothing.
    fn attention_of(pr: &PullRequest, viewer: &Username) -> Option<Attention> {
        attention(pr, viewer, &Seen::new())
    }

    fn reviewer(name: &str, state: ReviewerState) -> Reviewer {
        Reviewer {
            author: User {
                username: name.into(),
            },
            state,
        }
    }

    fn pr(author: &str, ci: CiSummary, reviewers: Vec<Reviewer>) -> PullRequest {
        PullRequest {
            url: None,
            id: PrId(1),
            title: "t".into(),
            description: None,
            author: User {
                username: author.into(),
            },
            ci,
            status: PrStatus::Open,
            reviewers,
            labels: Vec::new(),
            comment_count: 0,
            source_branch: "f".into(),
            target_branch: "main".into(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            created: Utc::now(),
            updated: Utc::now(),
            ai_review: crate::domain::pr::AiReview::None,
        }
    }

    #[test]
    fn a_requested_review_asks_the_reviewer_not_the_author() {
        let requested = pr(
            "alice",
            CiSummary::Success,
            vec![reviewer("me", ReviewerState::Requested)],
        );
        assert_eq!(
            attention_of(&requested, &"me".into()),
            Some(Attention::ReviewRequested)
        );
        assert_eq!(attention_of(&requested, &"alice".into()), None);
        assert_eq!(attention_of(&requested, &"someone-else".into()), None);
    }

    #[test]
    fn a_review_already_given_asks_nothing() {
        for state in [
            ReviewerState::Approved,
            ReviewerState::ChangesRequested,
            ReviewerState::Commented,
        ] {
            let given = pr("alice", CiSummary::Success, vec![reviewer("me", state)]);
            assert_eq!(attention_of(&given, &"me".into()), None);
        }
    }

    #[test]
    fn the_authors_blockers_rank_changes_requested_over_failed_ci() {
        let both = pr(
            "me",
            CiSummary::Failed,
            vec![reviewer("bob", ReviewerState::ChangesRequested)],
        );
        assert_eq!(
            attention_of(&both, &"me".into()),
            Some(Attention::ChangesRequested)
        );
        let ci_only = pr("me", CiSummary::Failed, Vec::new());
        assert_eq!(
            attention_of(&ci_only, &"me".into()),
            Some(Attention::CiFailed)
        );
    }

    #[test]
    fn approved_needs_every_reviewer_to_approve() {
        let all = pr(
            "me",
            CiSummary::Success,
            vec![
                reviewer("bob", ReviewerState::Approved),
                reviewer("carol", ReviewerState::Approved),
            ],
        );
        assert_eq!(attention_of(&all, &"me".into()), Some(Attention::Approved));
        let waiting = pr(
            "me",
            CiSummary::Success,
            vec![
                reviewer("bob", ReviewerState::Approved),
                reviewer("carol", ReviewerState::Requested),
            ],
        );
        assert_eq!(attention_of(&waiting, &"me".into()), None);
        assert_eq!(
            attention_of(&pr("me", CiSummary::Success, Vec::new()), &"me".into()),
            None
        );
    }

    #[test]
    fn only_open_prs_with_a_known_viewer_ask_anything() {
        let mut merged = pr("me", CiSummary::Failed, Vec::new());
        merged.status = PrStatus::Merged;
        assert_eq!(attention_of(&merged, &"me".into()), None);
        let mut draft = pr("me", CiSummary::Failed, Vec::new());
        draft.status = PrStatus::Draft;
        assert_eq!(attention_of(&draft, &"me".into()), None);
        // A deleted account has no name, and its PR is nobody's.
        assert_eq!(
            attention_of(&pr("", CiSummary::Failed, Vec::new()), &"me".into()),
            None
        );
    }

    #[test]
    fn usernames_compare_without_regard_to_case() {
        let requested = pr(
            "alice",
            CiSummary::Success,
            vec![reviewer("Me", ReviewerState::Requested)],
        );
        assert_eq!(
            attention_of(&requested, &"me".into()),
            Some(Attention::ReviewRequested)
        );
        assert_eq!(
            attention_of(&pr("ME", CiSummary::Failed, Vec::new()), &"me".into()),
            Some(Attention::CiFailed)
        );
    }

    #[test]
    fn urgency_order_is_the_declaration_order() {
        let mut all = [
            Attention::Approved,
            Attention::ReviewRequested,
            Attention::CiFailed,
            Attention::ChangesRequested,
        ];
        all.sort();
        assert_eq!(
            all.map(Attention::label),
            [
                "changes requested",
                "CI failed",
                "review requested",
                "approved"
            ]
        );
    }

    /// A reader who looked at the PR when it had `before` comments, with the PR
    /// as it is now having `now`.
    fn looked_at_with(before: u32, now: u32, mut pr: PullRequest) -> (PullRequest, Seen) {
        pr.comment_count = before;
        let mut seen = Seen::new();
        seen.look(&pr, pr.updated);
        pr.comment_count = now;
        pr.updated += chrono::Duration::hours(1);
        (pr, seen)
    }

    #[test]
    fn new_comments_since_the_reader_looked_are_a_reason_of_their_own() {
        let (pr, seen) = looked_at_with(2, 5, pr("someone", CiSummary::Unknown, Vec::new()));
        assert_eq!(
            attention(&pr, &"me".into(), &seen),
            Some(Attention::NewComments)
        );
        assert_eq!(Attention::NewComments.label(), "new comments");
    }

    #[test]
    fn the_roles_reasons_come_first_and_new_comments_is_the_least_urgent() {
        // My PR, the checks failed and there are new comments: the failure is the reason.
        let (mine, seen) = looked_at_with(0, 3, pr("me", CiSummary::Failed, Vec::new()));
        assert_eq!(
            attention(&mine, &"me".into(), &seen),
            Some(Attention::CiFailed)
        );
        for above in [
            Attention::ChangesRequested,
            Attention::CiFailed,
            Attention::ReviewRequested,
            Attention::Approved,
        ] {
            assert!(above < Attention::NewComments);
        }
    }

    #[test]
    fn nothing_new_nothing_opened_or_a_pr_that_is_over_is_no_reason() {
        let base = pr("someone", CiSummary::Unknown, Vec::new());
        // Never opened: more comments than nothing is not new.
        let mut unseen = base.clone();
        unseen.comment_count = 9;
        assert_eq!(attention(&unseen, &"me".into(), &Seen::new()), None);
        // Opened, no more comments than before.
        let (same, seen) = looked_at_with(4, 4, base.clone());
        assert_eq!(attention(&same, &"me".into(), &seen), None);
        // A merged PR asks for nothing.
        let (mut done, seen) = looked_at_with(1, 6, base);
        done.status = PrStatus::Merged;
        assert_eq!(attention(&done, &"me".into(), &seen), None);
    }
}
