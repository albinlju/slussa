use super::user::User;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: String,
    pub author: User,
    pub state: ReviewState,
    pub body: Option<String>,
}
