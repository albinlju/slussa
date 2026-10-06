use super::{authorship::Authorship, user::AccountKind};
use chrono::{DateTime, Utc};

/// A commit's full object id. A type of its own, so that it cannot be passed
/// where another string (a path, a cursor, a branch) is expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct CommitOid(String);

impl TryFrom<String> for CommitOid {
    type Error = String;

    /// What is read back from a file goes through the same rule as the rest.
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text).ok_or_else(|| "not a commit id".to_owned())
    }
}

impl CommitOid {
    /// A commit id as git writes it: hexadecimal digits, from the four an
    /// abbreviation has to the sixty-four of a SHA-256. It goes into the paths
    /// of requests, so nothing else is let in: the only way to make one.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = (4..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit());
        hex.then(|| Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The seven characters a commit is known by.
    pub fn short(&self) -> String {
        self.0.chars().take(7).collect()
    }

    /// Whether this is `full` written short: its start, and at least the seven
    /// characters a commit is known by. Fewer say too little about which commit
    /// was meant.
    pub fn abbreviates(&self, full: &Self) -> bool {
        self.0.len() >= 7
            && self.0.len() < full.0.len()
            && full
                .0
                .get(..self.0.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(&self.0))
    }
}

impl std::fmt::Display for CommitOid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
impl From<&str> for CommitOid {
    fn from(oid: &str) -> Self {
        Self(oid.to_owned())
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub oid: CommitOid,
    pub headline: String,
    /// The whole message, trailers included: an agent that commits under its
    /// owner's account says so there.
    pub message: String,
    pub author_name: String,
    /// Whose account made it, where the provider tells a bot's from a person's.
    pub account: AccountKind,
    /// Whether an agent made it. `Human` until the store has judged it.
    pub authorship: Authorship,
    pub authored_at: DateTime<Utc>,
    pub additions: u32,
    pub deletions: u32,
}

impl Commit {
    pub fn is_ai(&self) -> bool {
        self.authorship == Authorship::Ai
    }
}

#[cfg(test)]
mod tests {
    use super::CommitOid;

    #[test]
    fn a_commit_id_is_hexadecimal_and_nothing_else_is() {
        for ok in ["abcd", "ABCDEF12", &"a".repeat(40), &"0".repeat(64)] {
            assert!(CommitOid::parse(ok).is_some(), "{ok}");
        }
        for bad in [
            "",
            "abc",
            "../../x",
            "abcd/../x",
            "abcd?x=1",
            "abcd#",
            "abcd ",
            "abcg",
            "ååååå",
            &"a".repeat(65),
        ] {
            assert!(CommitOid::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn a_commit_written_short_is_the_start_of_the_whole_and_seven_characters_or_more() {
        let full = CommitOid::parse("9f2c1ab7d0e4455566677788899900aabbccddee").unwrap();
        let short = |text: &str| CommitOid::parse(text).unwrap();
        assert!(short("9f2c1ab").abbreviates(&full));
        assert!(short("9F2C1AB7D0").abbreviates(&full), "whatever the case");
        assert!(!short("9f2c1a").abbreviates(&full), "six say too little");
        assert!(!short("1ab7d0e").abbreviates(&full), "not its start");
        assert!(
            !full.abbreviates(&full),
            "the whole is not short for itself"
        );
    }
}
