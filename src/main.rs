#[allow(dead_code)]
mod app;
#[allow(dead_code)]
mod domain;
#[allow(dead_code)]
mod providers;
#[allow(dead_code)]
mod tui;

use app::{actions::refresh_prs, state::AppState};
use tui::run_tui;

fn main() {
    let mut state = AppState {
        prs: vec![],
        selected: 0,
        loading: true,
    };

    refresh_prs(&mut state);

    run_tui(state);
}
