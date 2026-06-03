use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderKind {
    GitHub,
    GitLab,
    Bitbucket,
    Gitea,
}
