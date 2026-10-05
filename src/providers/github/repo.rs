//! The repository every `gh` call acts on. It is read once when slussa starts
//! and handed to each call as `GH_REPO`, so that the diff that is read and the
//! merge that is sent can never go to two repositories that `gh` would have
//! picked between by its own rules.

use super::cli;

/// A GitHub repository, as `host/owner/name`. Only the constructors make one,
/// from parts that cannot break the address (`GH_REPO` is read by `gh` as a
/// path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhRepo {
    host: String,
    owner: String,
    name: String,
}

fn plain(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

impl GhRepo {
    pub fn new(host: &str, owner: &str, name: &str) -> Option<Self> {
        (plain(host) && plain(owner) && plain(name)).then(|| Self {
            host: host.to_owned(),
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    /// The repository a remote or a web address names: `https://host/owner/name`,
    /// `git@host:owner/name.git` or `ssh://git@host/owner/name`.
    pub fn from_remote(remote: &str) -> Option<Self> {
        let (authority, path) = crate::git_url::split(remote.trim())?;
        let host = authority.rsplit('@').next()?.split(':').next()?;
        let mut parts = path.split('/');
        let owner = parts.next()?;
        let name = parts.next()?;
        Self::new(host, owner, name.strip_suffix(".git").unwrap_or(name))
    }

    /// Which repository `gh` means here, asked once. A directory that `gh`
    /// cannot place (several remotes and no default) is placed by `origin`, as
    /// the drafts have always been. An answer on another host than `origin` is
    /// not used either: slussa talks to one host.
    pub fn resolve(origin: &str) -> Option<Self> {
        let origin_repo = Self::from_remote(origin)?;
        let asked = cli::run_gh_unpinned(&["repo", "view", "--json", "url", "--jq", ".url"])
            .ok()
            .and_then(|out| Self::from_remote(&String::from_utf8_lossy(&out)));
        match asked {
            Some(repo) if repo.host == origin_repo.host => {
                tracing::info!("gh places this directory in a repository");
                Some(repo)
            }
            Some(_) | None => {
                tracing::info!("gh did not place this directory; using the `origin` remote");
                Some(origin_repo)
            }
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    /// `owner/name`.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// Whether this is the repository `remote` names, as GitHub reads it: names
    /// are not case sensitive.
    pub fn is_remote(&self, remote: &str) -> bool {
        Self::from_remote(remote).is_some_and(|other| {
            self.host.eq_ignore_ascii_case(&other.host)
                && self.owner.eq_ignore_ascii_case(&other.owner)
                && self.name.eq_ignore_ascii_case(&other.name)
        })
    }

    #[cfg(test)]
    pub fn for_test() -> Self {
        Self::new("github.com", "octo", "repo").expect("a valid repository")
    }
}

impl std::fmt::Display for GhRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.host, self.owner, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeGh;

    #[test]
    fn a_remote_in_any_form_names_its_repository() {
        for remote in [
            "git@github.com:albinlju/slussa.git",
            "https://github.com/albinlju/slussa.git",
            "https://github.com/albinlju/slussa",
            "https://github.com/albinlju/slussa/pull/4",
            "ssh://git@github.com/albinlju/slussa.git",
            "ssh://git@github.com:22/albinlju/slussa.git",
        ] {
            let repo = GhRepo::from_remote(remote).unwrap_or_else(|| panic!("{remote}"));
            assert_eq!(repo.to_string(), "github.com/albinlju/slussa", "{remote}");
        }
    }

    #[test]
    fn what_is_not_a_repository_is_none() {
        for remote in [
            "",
            "not a remote",
            "https://github.com/onlyowner",
            "https://github.com//name",
            "https://github.com/ow ner/name",
            "https://github.com/owner/na;me",
            "https://github.com/owner/..%2Fother",
        ] {
            assert_eq!(GhRepo::from_remote(remote), None, "{remote:?}");
        }
    }

    #[test]
    fn a_remote_is_the_repository_whatever_the_case() {
        let repo = GhRepo::new("github.com", "albinlju", "slussa").unwrap();
        assert!(repo.is_remote("git@github.com:AlbinLju/Slussa.git"));
        assert!(!repo.is_remote("git@github.com:other/slussa.git"));
    }

    #[test]
    fn the_repository_gh_places_the_directory_in_is_the_one_used() {
        let _gh = FakeGh::new()
            .on("repo view", "https://github.com/upstream/slussa\n")
            .install();
        let repo = GhRepo::resolve("git@github.com:me/slussa.git").unwrap();
        assert_eq!(repo.to_string(), "github.com/upstream/slussa");
    }

    #[test]
    fn a_directory_gh_cannot_place_is_placed_by_origin() {
        let _gh = FakeGh::new()
            .fail("repo view", 1, "no default remote repository has been set")
            .install();
        let repo = GhRepo::resolve("git@github.com:me/slussa.git").unwrap();
        assert_eq!(repo.to_string(), "github.com/me/slussa");
    }

    #[test]
    fn an_answer_on_another_host_than_origin_is_not_used() {
        let _gh = FakeGh::new()
            .on("repo view", "https://ghe.example.com/team/slussa\n")
            .install();
        let repo = GhRepo::resolve("git@github.com:me/slussa.git").unwrap();
        assert_eq!(repo.to_string(), "github.com/me/slussa");
    }
}
