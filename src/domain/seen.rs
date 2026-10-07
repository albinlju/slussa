//! When the reader last looked at each PR, so that the list can mark the ones
//! that have changed since. Only PRs that have been opened can be unread: one
//! never opened has no point to be new since, and `Age` already says it is new.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::{
    commit::CommitOid,
    pr::{PrId, PullRequest},
    proposal::Proposal,
};

/// How long a PR may go unopened before it is forgotten, so that the file does
/// not grow with every PR ever looked at.
const KEEP: Duration = Duration::days(90);

/// How old a look may be before looking again renews its date. A look at an
/// unchanged PR is then written at most once a day, not at every refresh of the
/// list while it is on screen.
const RENEW: Duration = Duration::days(1);

/// What the reader did with an agent's proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum How {
    /// Taken into a comment of the reader's own, to edit and send.
    Taken,
    Discarded,
}

/// A proposal the reader has dealt with, kept whole so that the same one handed
/// in again is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Handled {
    proposal: Proposal,
    how: How,
}

/// What was known of a PR when it was last looked at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Look {
    /// The PR's `updated` then: later than this, something has happened.
    updated: DateTime<Utc>,
    /// When it was last looked at, to forget the ones not opened for a long time.
    at: DateTime<Utc>,
    /// The agent's proposals the reader has taken or discarded. A file from before
    /// they were kept has none, and one with none is written without the field,
    /// so the version 1 text stays as it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    handled: Vec<Handled>,
    /// The head of the diff the reader last had open, for what is new since. A
    /// file from before it was kept has none, and one with none is written
    /// without it, so the version 1 text stays as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head: Option<CommitOid>,
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
        let Some(look) = self.0.get_mut(&pr) else {
            self.0.insert(
                pr,
                Look {
                    updated,
                    at: now,
                    handled: Vec::new(),
                    head: None,
                },
            );
            return true;
        };
        if !(updated > look.updated || now - look.at >= RENEW) {
            return false;
        }
        look.updated = look.updated.max(updated);
        look.at = now;
        true
    }

    /// The reader took or discarded this proposal, at `now`. Returns whether the
    /// record changed. A PR not looked at yet gets its record here: the reader
    /// has it open, which is what a look is, even when the list has not said
    /// when it was updated.
    pub fn handle(&mut self, pr: PrId, proposal: &Proposal, how: How, now: DateTime<Utc>) -> bool {
        let look = self.0.entry(pr).or_insert_with(|| Look {
            updated: now,
            at: now,
            handled: Vec::new(),
            head: None,
        });
        if look
            .handled
            .iter()
            .any(|known| known.proposal.same_as(proposal))
        {
            return false;
        }
        look.handled.push(Handled {
            proposal: proposal.clone(),
            how,
        });
        true
    }

    /// The reader had the diff of `head` open. Returns whether the record
    /// changed, so that it is written. A PR never looked at has no record to
    /// put it in: it is opened first, and that is what makes one.
    pub fn mark_head(&mut self, pr: PrId, head: &CommitOid) -> bool {
        match self.0.get_mut(&pr) {
            Some(look) if look.head.as_ref() != Some(head) => {
                look.head = Some(head.clone());
                true
            }
            Some(_) | None => false,
        }
    }

    /// Whether the reader has already dealt with this proposal.
    pub fn is_handled(&self, pr: PrId, proposal: &Proposal) -> bool {
        self.0.get(&pr).is_some_and(|look| {
            look.handled
                .iter()
                .any(|known| known.proposal.same_as(proposal))
        })
    }

    /// The head of the diff the reader last had open, if one was kept.
    pub fn read_head(&self, pr: PrId) -> Option<&CommitOid> {
        self.0.get(&pr)?.head.as_ref()
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
            status: PrStatus::open(),
            reviewers: vec![],
            labels: vec![],
            comment_count: 0,
            source_branch: String::new(),
            source_repo: crate::domain::pr::SourceRepo::Unknown,
            head_oid: None,
            target_branch: String::new(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            created: at(0),
            updated,
            ai_review: crate::domain::pr::AiReview::None,
        }
    }

    fn proposal(line: usize) -> Proposal {
        Proposal::new(crate::domain::proposal::ProposalInput {
            head: "abc123".into(),
            path: "a.rs".into(),
            line,
            side: crate::domain::proposal::Side::New,
            body: "words".into(),
            id: None,
            agent: None,
        })
        .unwrap()
    }

    #[test]
    fn dealing_with_a_proposal_of_a_pr_not_looked_at_makes_its_record() {
        let mut seen = Seen::default();
        assert!(seen.handle(PrId(1), &proposal(1), How::Taken, at(5)));
        assert!(seen.is_handled(PrId(1), &proposal(1)));
        assert!(
            !seen.mark(PrId(1), at(4), at(5)),
            "the look is already there"
        );
    }

    #[test]
    fn a_proposal_dealt_with_is_known_again_and_a_later_look_does_not_forget_it() {
        let mut seen = Seen::default();
        seen.mark(PrId(1), at(1), at(2));
        assert!(!seen.is_handled(PrId(1), &proposal(1)));
        assert!(seen.handle(PrId(1), &proposal(1), How::Discarded, at(3)));
        assert!(
            !seen.handle(PrId(1), &proposal(1), How::Taken, at(3)),
            "once is enough"
        );
        assert!(seen.is_handled(PrId(1), &proposal(1)));
        assert!(
            !seen.is_handled(PrId(1), &proposal(2)),
            "another line is another one"
        );
        assert!(!seen.is_handled(PrId(2), &proposal(1)), "another PR");
        // Looking at the PR again moves its date and keeps what was decided.
        seen.mark(PrId(1), at(30), at(31));
        assert!(seen.is_handled(PrId(1), &proposal(1)));
    }

    #[test]
    fn the_head_read_is_kept_per_pr_and_survives_a_later_look() {
        let mut seen = Seen::default();
        let (first, second) = (CommitOid::from("abc123"), CommitOid::from("def456"));
        assert!(
            !seen.mark_head(PrId(1), &first),
            "a PR never opened has no record"
        );
        seen.mark(PrId(1), at(1), at(2));
        assert_eq!(seen.read_head(PrId(1)), None);
        assert!(seen.mark_head(PrId(1), &first));
        assert!(!seen.mark_head(PrId(1), &first), "the same head is no news");
        // A newer look at the PR does not forget which diff was read.
        seen.mark(PrId(1), at(3), at(4));
        assert_eq!(seen.read_head(PrId(1)), Some(&first));
        assert!(seen.mark_head(PrId(1), &second));
        assert_eq!(seen.read_head(PrId(1)), Some(&second));
        assert_eq!(seen.read_head(PrId(2)), None);
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
