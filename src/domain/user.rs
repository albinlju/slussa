#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
}

/// The account slussa acts as. Never empty: "is this mine?" compares against
/// it, and a provider gives a deleted account an empty name, which an empty
/// viewer would match. `parse` is the only way to make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username(String);

impl Username {
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        (!name.is_empty()).then(|| Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `name` is this account. Providers differ on case, so it is
    /// ignored.
    pub fn is(&self, name: &str) -> bool {
        self.0.eq_ignore_ascii_case(name)
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
impl From<&str> for Username {
    fn from(name: &str) -> Self {
        Self::parse(name).expect("a test user name is not empty")
    }
}

#[cfg(test)]
mod tests {
    use super::Username;

    #[test]
    fn a_user_name_is_never_empty_and_compares_without_case() {
        assert_eq!(Username::parse(""), None);
        assert_eq!(Username::parse("  \n"), None);
        let viewer = Username::parse(" Octocat\n").unwrap();
        assert_eq!(viewer.as_str(), "Octocat");
        assert!(viewer.is("octocat"));
        assert!(!viewer.is("octo"));
        // A deleted account has no name; that is never the viewer.
        assert!(!viewer.is(""));
    }
}
