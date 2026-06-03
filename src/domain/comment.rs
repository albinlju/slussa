use super::user::User;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub author: User,
    pub content: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub replies: Vec<Comment>,
    pub resolved: bool,
}
