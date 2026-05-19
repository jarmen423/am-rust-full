//! Agent chat — local fallback and optional HTTP provider stub.

use am_workspace::model::agent::{AgentChatRequest, AgentChatResponse, AgentProposal};
use std::env;

pub struct AgentService;

impl AgentService {
    pub fn chat(req: &AgentChatRequest, attempt_id: &str, request_id: &str) -> AgentChatResponse {
        let provider = if env::var("AM_AGENT_PROVIDER_URL").is_ok() {
            "http_stub"
        } else {
            "local_fallback"
        };

        let reply = if provider == "http_stub" {
            format!(
                "Provider URL configured but stub only: received {} chars. Set AM_AGENT_PROVIDER_URL to enable future HTTP wiring.",
                req.message.len()
            )
        } else {
            format!(
                "Local workspace assistant (no hosted MCP). You said: {}\n\nTip: select a note and ask for edits to see proposals.",
                req.message.chars().take(200).collect::<String>()
            )
        };

        let mut proposals = Vec::new();
        if req.note_id.is_some() && req.message.to_ascii_lowercase().contains("edit") {
            proposals.push(AgentProposal {
                proposal_id: format!("prop_{}", uuid::Uuid::new_v4().simple()),
                title: "Suggested note tweak".to_string(),
                summary: "Local fallback proposal — review before applying.".to_string(),
                suggested_markdown: Some(
                    "> Agent suggestion (local fallback)\n\n".to_string(),
                ),
                patch_preview: None,
            });
        }

        AgentChatResponse {
            status: "ok".to_string(),
            attempt_id: attempt_id.to_string(),
            request_id: request_id.to_string(),
            reply,
            provider: provider.to_string(),
            proposals,
        }
    }
}
