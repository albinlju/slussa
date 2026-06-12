use super::RepoCoords;

pub fn parse(remote: &str, host: &str) -> Option<RepoCoords> {
    let path = extract_path(remote)?;
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

    let (project, repo) = match parts.as_slice() {
        [proj, repo] | ["scm", proj, repo] => (*proj, *repo),
        _ => return None,
    };

    let repo_slug = repo.strip_suffix(".git").unwrap_or(repo).to_string();
    Some(RepoCoords {
        base_url: format!("https://{host}"),
        project_key: project.to_string(),
        repo_slug,
    })
}

fn extract_path(remote: &str) -> Option<&str> {
    if let Some(rest) = remote.strip_prefix("ssh://") {
        // ssh://user@host[:port]/path
        let (_authority, path) = rest.split_once('/')?;
        return Some(path);
    }
    if let Some(rest) = remote.strip_prefix("git@") {
        // user@host:path
        let (_authority, path) = rest.split_once(':')?;
        return Some(path);
    }
    if let Some(rest) = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
    {
        let (_authority, path) = rest.split_once('/')?;
        return Some(path);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_with_port() {
        let coords = parse(
            "ssh://git@bitbucket.kunden.se:7999/PLAT/payments-api.git",
            "bitbucket.kunden.se",
        )
        .unwrap();
        assert_eq!(coords.project_key, "PLAT");
        assert_eq!(coords.repo_slug, "payments-api");
        assert_eq!(coords.base_url, "https://bitbucket.kunden.se");
    }

    #[test]
    fn ssh_host_form() {
        let coords = parse(
            "git@bitbucket.kunden.se:PLAT/payments-api.git",
            "bitbucket.kunden.se",
        )
        .unwrap();
        assert_eq!(coords.project_key, "PLAT");
        assert_eq!(coords.repo_slug, "payments-api");
    }

    #[test]
    fn https_with_scm() {
        let coords = parse(
            "https://bitbucket.kunden.se/scm/PLAT/payments-api.git",
            "bitbucket.kunden.se",
        )
        .unwrap();
        assert_eq!(coords.project_key, "PLAT");
        assert_eq!(coords.repo_slug, "payments-api");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("not-a-url", "bitbucket.kunden.se").is_none());
    }
}
