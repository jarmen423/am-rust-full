//! Agent chat panel — local fallback, honest about provider.

use crate::api::{Promise, SharedPromise};
use crate::model::{AgentChatResponse, AgentProposal};
use egui::{ScrollArea, TextEdit, Ui};

#[derive(Debug, Default)]
pub struct AgentPanelState {
    pub message: String,
    pub transcript: Vec<(bool, String)>,
    pub proposals: Vec<AgentProposal>,
    pub provider_label: Option<String>,
    pub pending_apply: Option<String>,
}

pub struct AgentPanelOutput {
    pub send_chat: bool,
    pub apply_proposal_id: Option<String>,
}

impl Default for AgentPanelOutput {
    fn default() -> Self {
        Self {
            send_chat: false,
            apply_proposal_id: None,
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut AgentPanelState) -> AgentPanelOutput {
    let mut out = AgentPanelOutput::default();
    ui.heading("Workspace agent");
    ui.label(
        "Local fallback — not hosted MCP. Configure AM_AGENT_PROVIDER_URL on the server for future HTTP wiring.",
    );
    if let Some(ref p) = state.provider_label {
        ui.label(format!("Provider: {p}"));
    }
    ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            for (is_user, line) in &state.transcript {
                let prefix = if *is_user { "You" } else { "Agent" };
                ui.label(format!("{prefix}: {line}"));
            }
        });
    ui.add(
        TextEdit::multiline(&mut state.message)
            .desired_rows(3)
            .hint_text("Ask about the workspace or request an edit..."),
    );
    if ui.button("Send").clicked() && !state.message.trim().is_empty() {
        out.send_chat = true;
    }
    if !state.proposals.is_empty() {
        ui.separator();
        ui.label("Proposals:");
        for prop in &state.proposals {
            ui.horizontal(|ui| {
                ui.label(&prop.title);
                if ui.button("Apply").clicked() {
                    out.apply_proposal_id = Some(prop.proposal_id.clone());
                }
            });
            ui.label(prop.summary.as_str());
        }
    }
    out
}

pub fn poll_chat(
    promise: &SharedPromise<AgentChatResponse>,
    state: &mut AgentPanelState,
    user_message: &str,
) {
    let mut lock = promise.lock();
    if let Some(resp) = lock.take() {
        state.transcript.push((true, user_message.to_string()));
        state.transcript.push((false, resp.reply.clone()));
        state.provider_label = Some(resp.provider.clone());
        state.proposals = resp.proposals;
    } else if let Promise::Failed(e) = std::mem::replace(&mut *lock, Promise::Idle) {
        state.transcript.push((false, format!("Error: {e}")));
    }
}
