use crate::{app::state::AppState, providers::github};

pub fn refresh_prs(state: &mut AppState) {
    state.loading = true;

    let prs = github::fetch_prs();

    state.prs = prs;
    state.loading = false;
}
