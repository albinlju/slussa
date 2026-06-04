use crate::domain::pr::PullRequest;

#[derive(Debug, Default)]
pub struct AppState {
    pub prs: Vec<PullRequest>,
    pub selected: usize,
    pub mode: Mode,
    pub loading: bool,
    pub selected_pr: usize,
}

#[derive(Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    PrList,
}
