//! Agent chat and edit-proposal types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChatRequest {
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub message: String,
    pub note_id: Option<String>,
    pub board_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProposal {
    pub proposal_id: String,
    pub title: String,
    pub summary: String,
    pub suggested_markdown: Option<String>,
    pub patch_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChatResponse {
    pub status: String,
    pub attempt_id: String,
    pub request_id: String,
    pub reply: String,
    pub provider: String,
    pub proposals: Vec<AgentProposal>,
}
