use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub oid: String,
    pub headline: String,
    pub body: String,
    pub author_name: String,
    pub authored_at: DateTime<Utc>,
}
