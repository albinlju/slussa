pub mod pr_list;

use crate::app::state::AppState;

pub fn run_tui(_state: AppState) {
    // TODO: koppla in ratatui-loop
    //
    println!("{:?}", _state.prs)
}
