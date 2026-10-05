//! What the list's search field says: words to find in a PR's title, author or
//! number, and filters written `key:value`. The status (open, draft, merged)
//! is the list's own picker, `f`, so there is no filter for it here. A word that is not a filter is part
//! of the text, as before, so a title with a colon in it can still be searched
//! for. A known key with a value that is not one yet, as when it is half typed,
//! filters nothing, so the list narrows as the value is completed.

use super::{ci::CiSummary, pr::PullRequest, review::ReviewerState};

/// What a filter asks of a PR.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Filter {
    /// The author's name contains this, in lower case.
    Author(String),
    Review(ReviewIs),
    Ci(CiIs),
    Merge(MergeIs),
}

/// `review:` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewIs {
    /// Someone approved and nobody asked for changes.
    Approved,
    /// Someone asked for changes.
    Changes,
    /// A review is asked for and has not been given.
    Requested,
    /// There are no reviewers.
    None,
}

impl ReviewIs {
    const NAMES: [(&'static str, Self); 4] = [
        ("approved", Self::Approved),
        ("changes", Self::Changes),
        ("requested", Self::Requested),
        ("none", Self::None),
    ];

    fn holds(self, pr: &PullRequest) -> bool {
        let any = |state: ReviewerState| pr.reviewers.iter().any(|r| r.state == state);
        match self {
            Self::Approved => any(ReviewerState::Approved) && !any(ReviewerState::ChangesRequested),
            Self::Changes => any(ReviewerState::ChangesRequested),
            Self::Requested => any(ReviewerState::Requested),
            Self::None => pr.reviewers.is_empty(),
        }
    }
}

/// `merge:` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeIs {
    /// The provider says it cannot be merged for a conflict.
    Conflicts,
    /// The provider says there is no conflict. Says nothing of checks or
    /// reviews, and leaves out a PR whose conflict is not known.
    Clean,
}

impl MergeIs {
    const NAMES: [(&'static str, Self); 2] =
        [("conflicts", Self::Conflicts), ("clean", Self::Clean)];

    const fn holds(self, pr: &PullRequest) -> bool {
        match self {
            Self::Conflicts => pr.status.has_conflicts(),
            Self::Clean => pr.status.is_conflict_free(),
        }
    }
}

/// `ci:` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CiIs {
    Failed,
    Pending,
    Passing,
}

impl CiIs {
    const NAMES: [(&'static str, Self); 3] = [
        ("failed", Self::Failed),
        ("pending", Self::Pending),
        ("passing", Self::Passing),
    ];

    fn holds(self, pr: &PullRequest) -> bool {
        pr.ci
            == match self {
                Self::Failed => CiSummary::Failed,
                Self::Pending => CiSummary::Pending,
                Self::Passing => CiSummary::Success,
            }
    }
}

/// The value among `names` that `typed` is, or the only one it begins, so that
/// `review:app` is `review:approved` while it is being typed.
fn named<T: Copy>(typed: &str, names: &[(&str, T)]) -> Option<T> {
    if let Some((_, value)) = names.iter().find(|(name, _)| *name == typed) {
        return Some(*value);
    }
    let mut starting = names.iter().filter(|(name, _)| name.starts_with(typed));
    match (starting.next(), starting.next()) {
        (Some((_, value)), None) if !typed.is_empty() => Some(*value),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PrQuery {
    /// What is left of the input once the filters are taken out, in lower case.
    text: String,
    filters: Vec<Filter>,
}

impl PrQuery {
    pub fn parse(input: &str) -> Self {
        let mut words = Vec::new();
        let mut filters = Vec::new();
        for word in input.split_whitespace() {
            let lower = word.to_lowercase();
            match lower.split_once(':') {
                Some(("author", name)) => {
                    // A name not typed yet filters nothing.
                    if !name.is_empty() {
                        filters.push(Filter::Author(name.to_owned()));
                    }
                }
                Some(("review", value)) => {
                    filters.extend(named(value, &ReviewIs::NAMES).map(Filter::Review));
                }
                Some(("ci", value)) => {
                    filters.extend(named(value, &CiIs::NAMES).map(Filter::Ci));
                }
                Some(("merge", value)) => {
                    filters.extend(named(value, &MergeIs::NAMES).map(Filter::Merge));
                }
                _ => words.push(lower),
            }
        }
        Self {
            text: words.join(" "),
            filters,
        }
    }

    pub fn matches(&self, pr: &PullRequest) -> bool {
        let text_ok = self.text.is_empty()
            || pr.title.to_lowercase().contains(&self.text)
            || pr.author.username.to_lowercase().contains(&self.text)
            || format!("#{}", pr.id).contains(&self.text);
        text_ok && self.filters.iter().all(|filter| filter.holds(pr))
    }
}

impl Filter {
    fn holds(&self, pr: &PullRequest) -> bool {
        match self {
            Self::Author(name) => pr.author.username.to_lowercase().contains(name),
            Self::Review(review) => review.holds(pr),
            Self::Ci(ci) => ci.holds(pr),
            Self::Merge(merge) => merge.holds(pr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        pr::{Conflicts, OpenPr, PrStatus},
        review::Reviewer,
        user::User,
    };
    use chrono::Utc;

    fn pr(title: &str, author: &str) -> PullRequest {
        PullRequest {
            title: title.into(),
            author: User {
                username: author.into(),
            },
            ..PullRequest::for_test(42, Utc::now())
        }
    }

    fn reviewed(mut pr: PullRequest, states: &[ReviewerState]) -> PullRequest {
        pr.reviewers = states
            .iter()
            .map(|state| Reviewer {
                author: User {
                    username: "bob".into(),
                },
                state: state.clone(),
            })
            .collect();
        pr
    }

    fn matching(query: &str, pr: &PullRequest) -> bool {
        PrQuery::parse(query).matches(pr)
    }

    #[test]
    fn plain_words_search_the_title_the_author_and_the_number_as_before() {
        let pr = pr("Fix the retry loop", "Alice");
        assert!(matching("", &pr));
        assert!(matching("retry", &pr));
        assert!(matching("fix the", &pr), "the words together, as one text");
        assert!(matching("ALICE", &pr));
        assert!(matching("#42", &pr));
        assert!(!matching("deadlock", &pr));
    }

    #[test]
    fn author_narrows_to_a_name_and_combines_with_words() {
        let pr = pr("Fix the retry loop", "Alice");
        assert!(matching("author:ali", &pr));
        assert!(matching("author:ali retry", &pr));
        assert!(!matching("author:bob", &pr));
        assert!(!matching("author:ali deadlock", &pr));
        // Not typed yet: no filter.
        assert!(matching("author:", &pr));
    }

    #[test]
    fn review_tells_approved_from_changes_requested_requested_and_none() {
        let approved = reviewed(pr("A", "x"), &[ReviewerState::Approved]);
        let changes = reviewed(
            pr("B", "x"),
            &[ReviewerState::Approved, ReviewerState::ChangesRequested],
        );
        let waiting = reviewed(pr("C", "x"), &[ReviewerState::Requested]);
        let nobody = pr("D", "x");
        assert!(matching("review:approved", &approved));
        assert!(
            !matching("review:approved", &changes),
            "someone asked for changes"
        );
        assert!(matching("review:changes", &changes));
        assert!(matching("review:requested", &waiting));
        assert!(matching("review:none", &nobody) && !matching("review:none", &approved));
        // A prefix is the value while it is the only one that begins so.
        assert!(matching("review:app", &approved) && !matching("review:app", &waiting));
        assert!(matching("review:c", &changes), "c is changes alone");
    }

    #[test]
    fn ci_tells_failed_pending_and_passing() {
        let with = |ci| PullRequest { ci, ..pr("A", "x") };
        assert!(matching("ci:failed", &with(CiSummary::Failed)));
        assert!(matching("ci:pending", &with(CiSummary::Pending)));
        assert!(matching("ci:passing", &with(CiSummary::Success)));
        assert!(!matching("ci:passing", &with(CiSummary::Failed)));
        assert!(!matching("ci:failed", &with(CiSummary::Unknown)));
        assert!(matching("ci:f", &with(CiSummary::Failed)));
    }

    #[test]
    fn a_word_with_an_unknown_key_is_text_so_a_title_with_a_colon_can_be_found() {
        let colon = pr("fix: the retry loop", "alice");
        assert!(matching("fix:", &colon));
        assert!(!matching("fix:", &pr("Other", "alice")));
        // Filters and words together.
        assert!(matching("fix: ci:unknown_not_a_value", &colon));
    }

    #[test]
    fn merge_conflicts_keeps_the_prs_with_a_conflict() {
        let conflicting = PullRequest {
            status: PrStatus::conflicting(),
            ..pr("A", "x")
        };
        let clean = pr("B", "x");
        for typed in ["merge:conflicts", "merge:conf", "merge:co"] {
            assert!(matching(typed, &conflicting), "{typed}");
            assert!(!matching(typed, &clean), "{typed}");
        }
        assert!(matching("merge:", &clean), "nothing typed after the colon");
        assert!(
            matching("merge:c", &clean) && matching("merge:c", &conflicting),
            "c begins both values: nothing filtered until one is typed"
        );
        assert!(
            matching("merge:nonsense", &clean),
            "not a value: nothing filtered"
        );
    }

    #[test]
    fn merge_clean_keeps_the_prs_the_provider_says_have_no_conflict() {
        let status = |conflicts| PullRequest {
            status: PrStatus::Open(OpenPr {
                draft: false,
                conflicts,
            }),
            ..pr("A", "x")
        };
        let (no, yes, unknown) = (
            status(Conflicts::No),
            status(Conflicts::Yes),
            status(Conflicts::Unknown),
        );
        for typed in ["merge:clean", "merge:cl"] {
            assert!(matching(typed, &no), "{typed}");
            assert!(!matching(typed, &yes), "{typed}");
            assert!(
                !matching(typed, &unknown),
                "{typed}: not known to be free of conflicts"
            );
        }
        let merged = PullRequest {
            status: PrStatus::Merged,
            ..pr("B", "x")
        };
        assert!(
            !matching("merge:clean", &merged),
            "a merged PR has no conflict to be free of"
        );
    }
}
