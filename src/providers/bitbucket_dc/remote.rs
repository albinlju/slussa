use super::RepoLocation;

pub fn locate(remote: &str, host: &str) -> Option<RepoLocation> {
    let (_authority, path) = crate::git_url::split(remote)?;
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

    let (project, repo) = match parts.as_slice() {
        [proj, repo] | ["scm", proj, repo] => (*proj, *repo),
        _ => return None,
    };

    let repo_slug = repo.strip_suffix(".git").unwrap_or(repo).to_string();
    Some(RepoLocation {
        base_url: format!("https://{host}"),
        project_key: project.to_string(),
        repo_slug,
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
    fn rejects_garbage() {
        assert!(locate("not-a-url", "bitbucket.kunden.se").is_none());
    }
}
