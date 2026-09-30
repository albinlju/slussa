pub fn split(url: &str) -> Option<(&str, &str)> {
    if let Some(rest) = url.strip_prefix("ssh://") {
        return rest.split_once('/');
    }
    if let Some(rest) = url.strip_prefix("git@") {
        return rest.split_once(':');
    }
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?
        .split_once('/')
}

/// The web address a Bitbucket answers on, when the remote says so: the scheme,
/// host and port of an http(s) remote, plus the context path in front of
/// `scm/`. Ssh remotes carry no web address, so they give `None`.
pub fn web_base(url: &str) -> Option<String> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else {
        ("http", url.strip_prefix("http://")?)
    };
    let (authority, path) = rest.split_once('/')?;
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let segments: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let context = segments
        .iter()
        .rposition(|s| *s == "scm")
        .map_or(&segments[..0], |i| &segments[..i]);
    let mut base = format!("{scheme}://{authority}");
    for part in context {
        base.push('/');
        base.push_str(part);
    }
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_with_port() {
        assert_eq!(
            split("ssh://git@bitbucket.customer.com:7999/PLAT/payments.git"),
            Some(("git@bitbucket.customer.com:7999", "PLAT/payments.git"))
        );
    }

    #[test]
    fn scp_like() {
        assert_eq!(
            split("git@github.com:albinljung/tuipr.git"),
            Some(("github.com", "albinljung/tuipr.git"))
        );
    }

    #[test]
    fn https_with_scm() {
        assert_eq!(
            split("https://bitbucket.customer.com/scm/PLAT/payments.git"),
            Some(("bitbucket.customer.com", "scm/PLAT/payments.git"))
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(split("not-a-url"), None);
    }

    #[test]
    fn web_base_keeps_scheme_port_and_context() {
        assert_eq!(
            web_base("http://localhost:7990/scm/P/r.git").as_deref(),
            Some("http://localhost:7990")
        );
        assert_eq!(
            web_base("https://me@host/bitbucket/scm/P/r.git").as_deref(),
            Some("https://host/bitbucket")
        );
        assert_eq!(web_base("git@host:P/r.git"), None);
    }
}
