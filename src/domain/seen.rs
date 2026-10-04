//! When the reader last looked at each PR, so that the list can mark the ones
//! that have changed since. Only PRs that have been opened can be unread: one
//! never opened has no point to be new since, and `Age` already says it is new.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::pr::{PrId, PullRequest};

/// How long a PR may go unopened before it is forgotten, so that the file does
/// not grow with every PR ever looked at.
const KEEP: Duration = Duration::days(90);

/// How old a look may be before looking again renews its date. A look at an
/// unchanged PR is then written at most once a day, not at every refresh of the
/// list while it is on screen.
const RENEW: Duration = Duration::days(1);

/// What was known of a PR when it was last looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Look {
    /// The PR's `updated` then: later than this, something has happened.
    updated: DateTime<Utc>,
    /// When it was last looked at, to forget the ones not opened for a long time.
    at: DateTime<Utc>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seen(BTreeMap<PrId, Look>);

impl Seen {
    /// Nothing looked at.
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// The PR was looked at, and had been updated at `updated`. Returns whether
    /// the record changed, so that it is written: the state seen moves forward
    /// and never back, and the date of the look is renewed once it is a day old,
    /// so that a PR opened now and then is not forgotten as long as it is.
    /// `now` dates the look.
    pub fn mark(&mut self, pr: PrId, updated: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        let before = self.0.get(&pr).copied();
        let moved = before.is_none_or(|look| updated > look.updated);
        let old = before.is_some_and(|look| now - look.at >= RENEW);
        if !(moved || old) {
            return false;
        }
        let updated = before.map_or(updated, |look| look.updated.max(updated));
        self.0.insert(pr, Look { updated, at: now });
        true
    }

    /// Whether the PR was opened before and has been updated since.
    pub fn is_unread(&self, pr: &PullRequest) -> bool {
        self.0
            .get(&pr.id)
            .is_some_and(|look| pr.updated > look.updated)
    }

    /// Forget the looks older than the time kept, as of `now`.
    pub fn forget_old(&mut self, now: DateTime<Utc>) {
        self.0.retain(|_, look| now - look.at <= KEEP);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ci::CiSummary, pr::PrStatus, user::User};

    fn at(hour: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap() + Duration::hours(hour)
    }

    fn pr(id: u64, updated: DateTime<Utc>) -> PullRequest {
        PullRequest {
            url: None,
            id: PrId(id),
            title: String::new(),
            description: None,
            author: User {
                username: "alice".into(),
            },
            ci: CiSummary::Unknown,
            status: PrStatus::Open,
            reviewers: vec![],
            labels: vec![],
            comment_count: 0,
            source_branch: String::new(),
            target_branch: String::new(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            created: at(0),
            updated,
            ai_review: crate::domain::pr::AiReview::None,
            has_conflicts: false,
        }
    }

    #[test]
    fn a_pr_never_opened_is_never_unread() {
        assert!(!Seen::default().is_unread(&pr(1, at(5))));
    }

    #[test]
    fn a_pr_is_unread_once_it_is_updated_after_the_look_and_read_again_by_a_new_one() {
        let mut seen = Seen::default();
        assert!(seen.mark(PrId(1), at(1), at(2)), "the first look is news");
        assert!(!seen.is_unread(&pr(1, at(1))), "nothing since");
        assert!(seen.is_unread(&pr(1, at(3))), "updated since");
        // Looking at the newer state clears it; a look at the same state is no news.
        assert!(seen.mark(PrId(1), at(3), at(4)));
        assert!(!seen.is_unread(&pr(1, at(3))));
        assert!(!seen.mark(PrId(1), at(3), at(5)));
    }

    #[test]
    fn a_look_never_moves_backwards_and_the_prs_are_kept_apart() {
        let mut seen = Seen::default();
        seen.mark(PrId(1), at(5), at(6));
        assert!(
            !seen.mark(PrId(1), at(2), at(7)),
            "an older state is no news"
        );
        assert!(!seen.is_unread(&pr(1, at(5))));
        assert!(
            !seen.is_unread(&pr(2, at(9))),
            "another PR was never opened"
        );
    }

    #[test]
    fn a_pr_opened_now_and_then_is_not_forgotten_while_it_is_still_opened() {
        let day = |n: i64| at(24 * n);
        let mut seen = Seen::default();
        assert!(seen.mark(PrId(1), at(0), day(0)));
        // The same state looked at again within a day is no news and no write.
        assert!(!seen.mark(PrId(1), at(0), day(0) + Duration::hours(2)));
        // Opened again every month with nothing changed: the date is renewed.
        for month in 1..=4 {
            assert!(seen.mark(PrId(1), at(0), day(30 * month)), "month {month}");
        }
        seen.forget_old(day(130));
        assert_eq!(seen.len(), 1, "opened 10 days ago");
        // Not opened for more than ninety days since: gone.
        seen.forget_old(day(215));
        assert_eq!(seen.len(), 0);
    }

    #[test]
    fn looks_older_than_ninety_days_are_forgotten() {
        let mut seen = Seen::default();
        seen.mark(PrId(1), at(0), at(0));
        seen.mark(PrId(2), at(0), at(24 * 91));
        seen.forget_old(at(24 * 92));
        assert_eq!(seen.len(), 1);
        assert!(seen.is_unread(&pr(2, at(1))) && !seen.is_unread(&pr(1, at(1))));
    }
}
