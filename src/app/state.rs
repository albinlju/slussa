use crate::domain::pr::PullRequest;

pub struct AppState {
    pub prs: Vec<PullRequest>,
    pub selected: usize,
    pub loading: bool,
}
