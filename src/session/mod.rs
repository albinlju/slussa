//! Who and where: the provider for this repository and the account acting on
//! it. Shared by the TUI and the headless subcommands.

// A match on one of our own enums names every variant, so that adding one is a
// compile error wherever it has to be handled.
#![cfg_attr(not(test), warn(clippy::wildcard_enum_match_arm))]

pub mod preflight;
pub mod remote;

use crate::{domain::user::Username, providers::Provider};
use preflight::PreflightError;

/// What the TUI starts from: a provider that passed preflight and the account
/// it acts as. Only `connect` makes one, so the app never runs without either.
#[derive(Debug)]
pub struct Session {
    provider: Provider,
    user: Username,
}

impl Session {
    pub const fn provider(&self) -> &Provider {
        &self.provider
    }

    pub const fn user(&self) -> &Username {
        &self.user
    }

    pub fn into_parts(self) -> (Provider, Username) {
        (self.provider, self.user)
    }

    #[cfg(test)]
    pub fn for_test(provider: Provider, user: &str) -> Self {
        Self {
            provider,
            user: user.into(),
        }
    }
}

/// Find the provider for this repository and who is logged in to it.
pub fn connect() -> Result<Session, PreflightError> {
    let provider = preflight::run()?;
    // Asking who is logged in is also the first real request: a token that
    // the server no longer accepts is found out here.
    let user = provider
        .current_user()
        .map_err(PreflightError::AccountUnknown)?;
    Ok(Session { provider, user })
}
