//! Diagnostics and workflow attempt routes.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;

use am_workspace::model::diagnostics::{
    AttemptResponse, DiagnosticsHealthResponse, WorkflowAttempt, WorkflowState, WorkflowStepEvent,
};
use crate::observability::{new_attempt_id, new_diagnostic_id, new_request_id, new_trace_id};
use super::WorkspaceState;

/// Health snapshot for local diagnostics panel.
pub async fn diagnostics_health(
    State(state): State<Arc<WorkspaceState>>,
) -> Json<DiagnosticsHealthResponse> {
    Json(DiagnosticsHealthResponse {
        status: "ok".to_string(),
        ladybug_available: state.ladybug_db.is_some(),
        workspace_id: "default".to_string(),
        active_attempts: state.attempt_store.len(),
    })
}

/// Fetch a workflow attempt by id (user-safe fields).
pub async fn get_attempt(
    State(state): State<Arc<WorkspaceState>>,
    Path(attempt_id): Path<String>,
) -> Json<serde_json::Value> {
    match state.attempt_store.get(&attempt_id) {
        Some(attempt) => Json(serde_json::json!(AttemptResponse {
            status: "ok".to_string(),
            attempt,
        })),
        None => Json(serde_json::json!({
            "status": "error",
            "error_code": "not_found",
            "message": "Attempt not found",
        })),
    }
}

/// Create a heartbeat attempt (used by UI to verify diagnostics wiring).
pub async fn create_ping_attempt(
    State(state): State<Arc<WorkspaceState>>,
) -> Json<AttemptResponse> {
    let attempt = new_ping_attempt();
    state.attempt_store.insert(attempt.clone());
    Json(AttemptResponse {
        status: "ok".to_string(),
        attempt,
    })
}

pub fn new_ping_attempt() -> WorkflowAttempt {
    let now = Utc::now();
    let attempt_id = new_attempt_id();
    WorkflowAttempt {
        attempt_id: attempt_id.clone(),
        request_id: new_request_id(),
        trace_id: new_trace_id(),
        workflow_name: "diagnostics_ping".to_string(),
        workspace_id: "default".to_string(),
        project_id: None,
        state: WorkflowState::Completed,
        error_code: None,
        diagnostic_id: new_diagnostic_id(),
        user_message: Some("Diagnostics channel OK".to_string()),
        steps: vec![WorkflowStepEvent {
            step_name: "ping".to_string(),
            event: "step_finished".to_string(),
            duration_ms: Some(0),
            error_code: None,
            message: None,
            at: now,
        }],
        created_at: now,
        updated_at: now,
    }
}
