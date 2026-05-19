//! Read-only Cypher query route for operator shell.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;

use am_workspace::model::diagnostics::{
    WorkflowAttempt, WorkflowState, WorkflowStepEvent,
};
use crate::observability::{new_attempt_id, new_diagnostic_id, new_request_id, new_trace_id};
use crate::services::ladybug_query::{LadybugQueryService, QueryClassification, DEFAULT_ROW_LIMIT};

use super::WorkspaceState;

#[derive(Debug, Deserialize)]
pub struct QueryBody {
    pub cypher: String,
    pub limit: Option<usize>,
    pub workspace_id: Option<String>,
}

pub async fn execute_query(
    State(state): State<Arc<WorkspaceState>>,
    Json(body): Json<QueryBody>,
) -> Json<serde_json::Value> {
    let request_id = new_request_id();
    let attempt_id = new_attempt_id();
    let diagnostic_id = new_diagnostic_id();
    let now = Utc::now();
    let row_limit = body.limit.unwrap_or(DEFAULT_ROW_LIMIT).min(500);

    let attempt = WorkflowAttempt {
        attempt_id: attempt_id.clone(),
        request_id: request_id.clone(),
        trace_id: new_trace_id(),
        workflow_name: "graph_query".to_string(),
        workspace_id: body
            .workspace_id
            .unwrap_or_else(|| "default".to_string()),
        project_id: None,
        state: WorkflowState::Requested,
        error_code: None,
        diagnostic_id: diagnostic_id.clone(),
        user_message: None,
        steps: vec![WorkflowStepEvent {
            step_name: "validate".to_string(),
            event: "step_started".to_string(),
            duration_ms: None,
            error_code: None,
            message: None,
            at: now,
        }],
        created_at: now,
        updated_at: now,
    };
    state.attempt_store.insert(attempt.clone());

    let result = LadybugQueryService::execute(state.ladybug_db.as_ref(), &body.cypher, row_limit);

    match result {
        Ok(qr) => {
            let classification = match qr.classification {
                QueryClassification::ReadOnly => "readonly",
                QueryClassification::RejectedMutation => "rejected_mutation",
                QueryClassification::Empty => "empty",
            };
            if qr.classification == QueryClassification::RejectedMutation {
                state.attempt_store.fail(
                    &attempt_id,
                    "query_not_readonly",
                    "Only read-only Cypher is allowed",
                );
                return Json(serde_json::json!({
                    "status": "error",
                    "error_code": "query_not_readonly",
                    "diagnostic_id": diagnostic_id,
                    "request_id": request_id,
                    "attempt_id": attempt_id,
                    "message": "Only read-only Cypher is allowed (MATCH, RETURN, etc.)",
                }));
            }
            state.attempt_store.set_state(&attempt_id, WorkflowState::Completed);
            Json(serde_json::json!({
                "status": "ok",
                "request_id": request_id,
                "attempt_id": attempt_id,
                "diagnostic_id": diagnostic_id,
                "classification": classification,
                "columns": qr.columns,
                "rows": qr.rows,
                "row_count": qr.rows.len(),
                "truncated": qr.truncated,
                "duration_ms": qr.duration_ms,
            }))
        }
        Err(code) => {
            let message = match code.as_str() {
                "ladybug_unavailable" => "Ladybug database is not available",
                "query_limit_exceeded" => "Query text is too long",
                _ => "Query failed",
            };
            state.attempt_store.fail(&attempt_id, &code, message);
            Json(serde_json::json!({
                "status": "error",
                "error_code": code,
                "diagnostic_id": diagnostic_id,
                "request_id": request_id,
                "attempt_id": attempt_id,
                "message": message,
            }))
        }
    }
}
