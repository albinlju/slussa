use super::provider::ProviderKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub remote_url: String,
    pub provider: ProviderKind,
}
