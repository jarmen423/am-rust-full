pub mod agent;
pub mod canvas;
pub mod diagnostics;
pub mod graph;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Core note type — persisted as JSON in workspace-store/
/// and as Markdown with frontmatter in workspace-vaults/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceNoteDocument {
    pub note_id: String,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub slug: String,
    pub title: String,
    pub body_markdown: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub entity_hints: Vec<String>,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub graph_status: String,
    pub markdown_path: Option<String>,
    /// Populated at runtime, not persisted in JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_revision: Option<String>,
}

/// Core board type — persisted as JSON in workspace-store/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceBoard {
    pub board_id: String,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub board_type: String,
    pub board_state: String,
    pub tldraw_document: serde_json::Value,
    pub objects: Vec<WorkspaceBoardObject>,
    pub connectors: Vec<WorkspaceConnector>,
    pub object_count: i32,
    pub connector_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub ingested_at: Option<DateTime<Utc>>,
    pub graph_status: String,
}

/// Object embedded in a board (note card, text card, graph reference)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceBoardObject {
    pub object_id: String,
    pub board_id: String,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub object_type: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub note_id: Option<String>,
    pub asset_id: Option<String>,
    pub artifact_id: Option<String>,
    pub graph_entity_name: Option<String>,
    pub graph_source_id: Option<String>,
    pub tags: Vec<String>,
    pub ingest_eligible: bool,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Connector between two board objects
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceConnector {
    pub connector_id: String,
    pub board_id: String,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub from_object_id: String,
    pub to_object_id: String,
    pub connector_type: String,
    pub relation_intent: String,
    pub custom_label: Option<String>,
    pub user_authored_summary: Option<String>,
    pub ingest_eligible: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Ingest payload — result of ingesting a board or selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceIngestPayload {
    pub ingest_id: String,
    pub ingest_scope: String,
    pub board_id: String,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub maturity: String,
    pub object_ids: Vec<String>,
    pub connector_ids: Vec<String>,
    pub object_count: i32,
    pub connector_count: i32,
    pub entity_count: i32,
    pub relation_count: i32,
    pub graph_status: String,
    pub ingested_at: DateTime<Utc>,
}

// ── API Response Wrappers ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteListResponse {
    pub status: String,
    pub notes: Vec<WorkspaceNoteDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteResponse {
    pub status: String,
    pub note: WorkspaceNoteDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardListResponse {
    pub status: String,
    pub boards: Vec<WorkspaceBoard>,
}

/// Board payload includes the board + its inline objects/connectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardDocumentPayload {
    pub status: String,
    pub board: WorkspaceBoard,
    pub objects: Vec<WorkspaceBoardObject>,
    pub connectors: Vec<WorkspaceConnector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteHistoryItem {
    pub sha: String,
    pub timestamp: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteHistoryResponse {
    pub status: String,
    pub items: Vec<NoteHistoryItem>,
}

// ── Bootstrap / Config ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFeatureFlags {
    pub notes: bool,
    pub boards: bool,
    pub connectors: bool,
    pub graph_view: bool,
    pub manual_ingest: bool,
    pub auto_ingest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub default_project_id: Option<String>,
    pub features: WorkspaceFeatureFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceBootstrapPayload {
    pub status: String,
    pub workspace: WorkspaceInfo,
}

// ── Re-exports ─────────────────────────────────────────────────────

pub use agent::*;
pub use canvas::*;
pub use diagnostics::*;
pub use graph::*;
