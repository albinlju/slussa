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
}
