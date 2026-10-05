//! What a pull request asks of the person looking at the list.
//!
//! Derived only from data the list already has: the viewer, the author, the
//! reviewers' states and the CI summary. Activity since the viewer last looked
//! and mentions are not here yet; they need local state.

use super::{ci::CiSummary, pr::PullRequest, review::ReviewerState, user::Username};

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
}

impl Attention {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChangesRequested => "changes requested",
            Self::CiFailed => "CI failed",
            Self::ReviewRequested => "review requested",
            Self::Approved => "approved",
        }
    }
}

/// The reason `pr` needs `viewer`, if any. Only open PRs ask for anything.
/// Usernames compare without regard to case, as providers differ on that.
pub fn attention(pr: &PullRequest, viewer: &Username) -> Option<Attention> {
    if !pr.status.is_ready() {
        return None;
    }
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
    use crate::domain::{
        pr::{PrId, PrStatus},
        review::Reviewer,
        user::User,
    };
    use chrono::Utc;

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
            status: PrStatus::open(),
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
            attention(&requested, &"me".into()),
            Some(Attention::ReviewRequested)
        );
        assert_eq!(attention(&requested, &"alice".into()), None);
        assert_eq!(attention(&requested, &"someone-else".into()), None);
    }

    #[test]
    fn a_review_already_given_asks_nothing() {
        for state in [
            ReviewerState::Approved,
            ReviewerState::ChangesRequested,
            ReviewerState::Commented,
        ] {
            let given = pr("alice", CiSummary::Success, vec![reviewer("me", state)]);
            assert_eq!(attention(&given, &"me".into()), None);
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
            attention(&both, &"me".into()),
            Some(Attention::ChangesRequested)
        );
        let ci_only = pr("me", CiSummary::Failed, Vec::new());
        assert_eq!(attention(&ci_only, &"me".into()), Some(Attention::CiFailed));
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
        assert_eq!(attention(&all, &"me".into()), Some(Attention::Approved));
        let waiting = pr(
            "me",
            CiSummary::Success,
            vec![
                reviewer("bob", ReviewerState::Approved),
                reviewer("carol", ReviewerState::Requested),
            ],
        );
        assert_eq!(attention(&waiting, &"me".into()), None);
        assert_eq!(
            attention(&pr("me", CiSummary::Success, Vec::new()), &"me".into()),
            None
        );
    }

    #[test]
    fn only_open_prs_with_a_known_viewer_ask_anything() {
        let mut merged = pr("me", CiSummary::Failed, Vec::new());
        merged.status = PrStatus::Merged;
        assert_eq!(attention(&merged, &"me".into()), None);
        let mut draft = pr("me", CiSummary::Failed, Vec::new());
        draft.status = PrStatus::draft();
        assert_eq!(attention(&draft, &"me".into()), None);
        // A deleted account has no name, and its PR is nobody's.
        assert_eq!(
            attention(&pr("", CiSummary::Failed, Vec::new()), &"me".into()),
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
            attention(&requested, &"me".into()),
            Some(Attention::ReviewRequested)
        );
        assert_eq!(
            attention(&pr("ME", CiSummary::Failed, Vec::new()), &"me".into()),
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
}
