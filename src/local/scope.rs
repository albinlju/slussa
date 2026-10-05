//! The name a saved file is filed under: one per provider, host, repository and
//! account.
//!
//! The repository is the one slussa acts on. For GitHub that is the one `gh`
//! places the directory in, which is `origin`'s unless the person works from a
//! fork and has told `gh` to use the upstream. Where the two are the same, the
//! name is spelled as it always was, so nothing saved before is hidden.

use std::io;

use crate::{domain::user::Username, providers::Provider};

/// What names a scope: provider, host and `owner/repo`.
struct Parts {
    provider: &'static str,
    host: String,
    repo: String,
}

/// What `origin` names, which is how a file was filed before the repository
/// was fixed.
fn origin_parts(provider: &Provider, remote: &str) -> io::Result<Parts> {
    let (authority, repo) = crate::git_url::split(remote)
        .ok_or_else(|| io::Error::other("Cannot identify repository for drafts"))?;
    let host = authority.rsplit('@').next().unwrap_or(authority);
    Ok(Parts {
        provider: match provider {
            Provider::GitHub(_) => "github",
            Provider::BitbucketDc(_) => "bitbucket-dc",
        },
        host: host.to_owned(),
        repo: repo.trim_end_matches(".git").to_owned(),
    })
}

fn name(parts: &Parts, user: &Username) -> io::Result<String> {
    Ok(serde_json::to_string(&(
        parts.provider,
        &parts.host,
        &parts.repo,
        user.as_str(),
    ))?)
}

pub fn scope(provider: &Provider, remote: &str, user: &Username) -> io::Result<String> {
    let origin = origin_parts(provider, remote)?;
    match provider {
        // A fork's person who acts on the upstream: that repository's own name.
        Provider::GitHub(repo) if !repo.is_remote(remote) => name(
            &Parts {
                provider: origin.provider,
                host: repo.host().to_owned(),
                repo: repo.slug(),
            },
            user,
        ),
        Provider::GitHub(_) | Provider::BitbucketDc(_) => name(&origin, user),
    }
}

/// The name a file was filed under before the repository was fixed, when it is
/// not the name now: what to look under for what was saved from a fork.
pub fn earlier_scope(
    provider: &Provider,
    remote: &str,
    user: &Username,
) -> io::Result<Option<String>> {
    let now = scope(provider, remote, user)?;
    let before = name(&origin_parts(provider, remote)?, user)?;
    Ok((before != now).then_some(before))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::GhRepo;

    fn github(owner: &str, name: &str) -> Provider {
        Provider::GitHub(GhRepo::new("github.com", owner, name).unwrap())
    }

    /// The scope names the saved file, so a change to it would hide everything
    /// saved before.
    #[test]
    fn the_scope_names_provider_host_repository_and_account() {
        let user = Username::parse("octocat").unwrap();
        let expected = r#"["github","github.com","albinlju/slussa","octocat"]"#;
        for remote in [
            "git@github.com:albinlju/slussa.git",
            "https://github.com/albinlju/slussa.git",
            "https://github.com/albinlju/slussa",
        ] {
            assert_eq!(
                scope(&github("albinlju", "slussa"), remote, &user).unwrap(),
                expected,
                "{remote}"
            );
        }
        assert!(scope(&github("albinlju", "slussa"), "not a remote", &user).is_err());
    }

    #[test]
    fn the_name_stays_as_origin_spells_it_where_the_repository_is_origins() {
        let user = Username::parse("octocat").unwrap();
        // GitHub reads names without case, and `origin` may spell them any way.
        let remote = "git@github.com:AlbinLju/Slussa.git";
        assert_eq!(
            scope(&github("albinlju", "slussa"), remote, &user).unwrap(),
            r#"["github","github.com","AlbinLju/Slussa","octocat"]"#
        );
        assert_eq!(
            earlier_scope(&github("albinlju", "slussa"), remote, &user).unwrap(),
            None
        );
    }

    #[test]
    fn a_fork_that_acts_on_the_upstream_is_filed_under_the_upstream_and_remembers_where_it_was() {
        let user = Username::parse("octocat").unwrap();
        let remote = "git@github.com:me/slussa.git";
        let provider = github("upstream", "slussa");
        assert_eq!(
            scope(&provider, remote, &user).unwrap(),
            r#"["github","github.com","upstream/slussa","octocat"]"#
        );
        assert_eq!(
            earlier_scope(&provider, remote, &user).unwrap().as_deref(),
            Some(r#"["github","github.com","me/slussa","octocat"]"#)
        );
    }
}
