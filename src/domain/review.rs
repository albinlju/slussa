use super::user::User;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewerState {
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reviewer {
    pub id: String,
    pub author: User,
    pub state: ReviewerState,
    pub body: Option<String>,
}
