//! Repo/project scope resolution for graph and workspace APIs.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScopeFilter {
    pub repo_id: Option<String>,
    pub project_id: Option<String>,
}

impl ScopeFilter {
    pub fn is_empty(&self) -> bool {
        self.repo_id.is_none() && self.project_id.is_none()
    }
}
