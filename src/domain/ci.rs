use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CiState {
    Pending,
    Success,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiStatus {
    pub state: CiState,
    pub description: Option<String>,
    pub url: Option<String>,
}
