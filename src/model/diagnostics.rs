//! Shared diagnostics and workflow attempt types (API + UI).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// High-level workflow state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    Requested,
    Validated,
    ContextLoaded,
    Executed,
    Persisted,
    Verified,
    Completed,
    Failed,
}

/// A single step within an attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStepEvent {
    pub step_name: String,
    pub event: String,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub at: DateTime<Utc>,
}

/// Durable attempt record returned to clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowAttempt {
    pub attempt_id: String,
    pub request_id: String,
    pub trace_id: String,
    pub workflow_name: String,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub state: WorkflowState,
    pub error_code: Option<String>,
    pub diagnostic_id: String,
    pub user_message: Option<String>,
    pub steps: Vec<WorkflowStepEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptResponse {
    pub status: String,
    pub attempt: WorkflowAttempt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsHealthResponse {
    pub status: String,
    pub ladybug_available: bool,
    pub workspace_id: String,
    pub active_attempts: usize,
}

/// User-safe error payload for failed API calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub status: String,
    pub error_code: String,
    pub diagnostic_id: String,
    pub message: String,
    pub request_id: Option<String>,
    pub attempt_id: Option<String>,
}
