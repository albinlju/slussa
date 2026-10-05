//! `slussa auth login`: stores a Bitbucket Data Center token for this repository's host.

use std::process::ExitCode;

use crate::{
    providers::bitbucket_dc,
    session::{preflight, remote},
};

pub(super) fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("login") | None => {
            let result = remote::origin_url()
                .map_err(|e| e.to_string())
                .and_then(|url| {
                    let host = remote::parse_host(&url)
                        .ok_or_else(|| format!("couldn't read a host from `{url}`"))?;
                    if let Some(message) = preflight::login_redirect(&host) {
                        return Err(message);
                    }
                    let base = bitbucket_dc::remote::base_url(&url, &host);
                    bitbucket_dc::auth::login(&host, &base)
                });
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("slussa: {}", crate::domain::printable::printable(&err));
                    ExitCode::from(1)
                }
            }
        }
        Some(other) => {
            eprintln!("slussa: unknown auth subcommand `{other}`. Try `slussa auth login`.");
            ExitCode::from(2)
        }
    }
}
