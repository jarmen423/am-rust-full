//! Agent chat route — local fallback / optional provider stub.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use chrono::Utc;

use am_workspace::model::agent::AgentChatRequest;
use am_workspace::model::diagnostics::{WorkflowAttempt, WorkflowState, WorkflowStepEvent};
use crate::observability::{new_attempt_id, new_diagnostic_id, new_request_id, new_trace_id};
use crate::services::agent::AgentService;

use super::WorkspaceState;

pub async fn agent_chat(
    State(state): State<Arc<WorkspaceState>>,
    Json(req): Json<AgentChatRequest>,
) -> Json<serde_json::Value> {
    let request_id = new_request_id();
    let attempt_id = new_attempt_id();
    let now = Utc::now();

    let attempt = WorkflowAttempt {
        attempt_id: attempt_id.clone(),
        request_id: request_id.clone(),
        trace_id: new_trace_id(),
        workflow_name: "agent_chat".to_string(),
        workspace_id: req.workspace_id.clone(),
        project_id: req.project_id.clone(),
        state: WorkflowState::Requested,
        error_code: None,
        diagnostic_id: new_diagnostic_id(),
        user_message: None,
        steps: vec![WorkflowStepEvent {
            step_name: "context_summarized".to_string(),
            event: "step_started".to_string(),
            duration_ms: None,
            error_code: None,
            message: None,
            at: now,
        }],
        created_at: now,
        updated_at: now,
    };
    state.attempt_store.insert(attempt);

    let response = AgentService::chat(&req, &attempt_id, &request_id);
    state
        .attempt_store
        .set_state(&attempt_id, WorkflowState::Completed);

    Json(serde_json::to_value(response).unwrap_or(serde_json::json!({
        "status": "error",
        "error_code": "internal_error",
        "message": "Failed to serialize agent response",
    })))
}
