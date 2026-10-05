use super::RepoLocation;

/// Where the web and REST address of the server is: what an http(s) remote
/// says, otherwise https on the remote's host.
pub fn base_url(remote: &str, host: &str) -> String {
    crate::git_url::web_base(remote).unwrap_or_else(|| format!("https://{host}"))
}

pub fn locate(remote: &str, host: &str) -> Option<RepoLocation> {
    let (_authority, path) = crate::git_url::split(remote)?;
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

    let (project, repo) = match parts.as_slice() {
        [proj, repo] | [.., "scm", proj, repo] => (*proj, *repo),
        _ => return None,
    };

    let repo_slug = repo.strip_suffix(".git").unwrap_or(repo);
    // Both go into paths as they are, so they are kept to their place in them.
    Some(RepoLocation {
        base_url: base_url(remote, host),
        project_key: crate::providers::url_path::segment(project),
        repo_slug: crate::providers::url_path::segment(repo_slug),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_with_port() {
        let repo = locate(
            "ssh://git@bitbucket.kunden.se:7999/PLAT/payments-api.git",
            "bitbucket.kunden.se",
        )
        .unwrap();
        assert_eq!(repo.project_key, "PLAT");
        assert_eq!(repo.repo_slug, "payments-api");
        assert_eq!(repo.base_url, "https://bitbucket.kunden.se");
    }

    #[test]
    fn ssh_host_form() {
        let repo = locate(
            "git@bitbucket.kunden.se:PLAT/payments-api.git",
            "bitbucket.kunden.se",
        )
        .unwrap();
        assert_eq!(repo.project_key, "PLAT");
        assert_eq!(repo.repo_slug, "payments-api");
    }

    #[test]
    fn https_with_scm() {
        let repo = locate(
            "https://bitbucket.kunden.se/scm/PLAT/payments-api.git",
            "bitbucket.kunden.se",
        )
        .unwrap();
        assert_eq!(repo.project_key, "PLAT");
        assert_eq!(repo.repo_slug, "payments-api");
    }

    #[test]
    fn http_with_port_keeps_scheme_and_port() {
        let repo = locate("http://localhost:7990/scm/PLAT/api.git", "localhost:7990").unwrap();
        assert_eq!(repo.base_url, "http://localhost:7990");
        assert_eq!(repo.project_key, "PLAT");
    }

    #[test]
    fn context_path_is_part_of_the_base_url() {
        let repo = locate("https://host.se/bitbucket/scm/PLAT/api.git", "host.se").unwrap();
        assert_eq!(repo.base_url, "https://host.se/bitbucket");
        assert_eq!(repo.project_key, "PLAT");
        assert_eq!(repo.repo_slug, "api");
    }

    #[test]
    fn a_name_that_is_not_plain_is_kept_to_its_place_in_a_path() {
        let repo = locate("git@host.se:PLAT/we ird#name?.git", "host.se").unwrap();
        assert_eq!(repo.repo_slug, "we%20ird%23name%3F");
        // A personal project's key keeps its tilde.
        let personal = locate("https://host.se/scm/~jsmith/api.git", "host.se").unwrap();
        assert_eq!(personal.project_key, "~jsmith");
    }

    #[test]
    fn rejects_garbage() {
        assert!(locate("not-a-url", "bitbucket.kunden.se").is_none());
    }
}
