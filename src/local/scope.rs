//! The name a saved file is filed under: one per provider, host, repository and
//! account.

use std::io;

use crate::{domain::user::Username, providers::Provider};

pub fn scope(provider: &Provider, remote: &str, user: &Username) -> io::Result<String> {
    let (authority, repo) = crate::git_url::split(remote)
        .ok_or_else(|| io::Error::other("Cannot identify repository for drafts"))?;
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let provider = match provider {
        Provider::GitHub => "github",
        Provider::BitbucketDc(_) => "bitbucket-dc",
    };
    Ok(serde_json::to_string(&(
        provider,
        host,
        repo.trim_end_matches(".git"),
        user.as_str(),
    ))?)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                scope(&Provider::GitHub, remote, &user).unwrap(),
                expected,
                "{remote}"
            );
        }
        assert!(scope(&Provider::GitHub, "not a remote", &user).is_err());
    }
}
