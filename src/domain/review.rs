use super::user::User;

#[derive(Debug, Clone, PartialEq)]
pub enum ReviewerState {
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Debug, Clone)]
pub struct Reviewer {
    pub author: User,
    pub state: ReviewerState,
}
