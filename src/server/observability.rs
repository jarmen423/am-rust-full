//! Correlation IDs and structured logging helpers for workspace-server.

use uuid::Uuid;

/// New HTTP-scoped request id.
pub fn new_request_id() -> String {
    format!("req_{}", Uuid::new_v4())
}

/// New workflow attempt id.
pub fn new_attempt_id() -> String {
    format!("att_{}", Uuid::new_v4())
}

/// Operator trace id (may equal request_id for local server).
pub fn new_trace_id() -> String {
    format!("tr_{}", Uuid::new_v4())
}

/// User-facing diagnostic id for support lookup.
pub fn new_diagnostic_id() -> String {
    format!("diag_{}", Uuid::new_v4().simple())
}
