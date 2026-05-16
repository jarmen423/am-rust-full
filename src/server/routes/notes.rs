//! Note CRUD route handlers.
//!
//! Provides: bootstrap, list, create, get, update, history, revert, picker.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use tracing::{debug, error, info, instrument, warn};

use am_workspace::model::*;
use crate::store;

use super::WorkspaceState;

// ── Request Bodies ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
    pub workspace_id: String,
    pub title: String,
    pub body_markdown: String,
    pub tags: Option<Vec<String>>,
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNoteRequest {
    pub workspace_id: String,
    pub title: String,
    pub body_markdown: String,
    pub tags: Option<Vec<String>>,
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RevertNoteRequest {
    pub workspace_id: String,
    pub revision: String,
}

// ── Handlers ───────────────────────────────────────────────────────

/// Bootstrap — return workspace info and feature flags.
#[instrument(skip(_state))]
pub async fn bootstrap(
    State(_state): State<Arc<WorkspaceState>>,
) -> Json<WorkspaceBootstrapPayload> {
    info!("bootstrap called");
    Json(WorkspaceBootstrapPayload {
        status: "ok".to_string(),
        workspace: WorkspaceInfo {
            workspace_id: "default".to_string(),
            default_project_id: None,
            features: WorkspaceFeatureFlags {
                notes: true,
                boards: true,
                connectors: true,
                graph_view: false, // Phase 4
                manual_ingest: true,
                auto_ingest: false,
            },
        },
    })
}

/// List all notes for the default workspace.
#[instrument(skip(state))]
pub async fn list_notes(State(state): State<Arc<WorkspaceState>>) -> Json<NoteListResponse> {
    let workspace_id = "default";
    debug!(workspace_id, "listing notes");

    match store::list_notes(
        &state.config.store_path,
        &state.config.vault_path,
        workspace_id,
    ) {
        Ok(notes) => {
            info!(count = notes.len(), "notes listed");
            Json(NoteListResponse {
                status: "ok".to_string(),
                notes,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to list notes");
            Json(NoteListResponse {
                status: "error".to_string(),
                notes: vec![],
            })
        }
    }
}

/// Create a new note.
#[instrument(skip(state, req))]
pub async fn create_note(
    State(state): State<Arc<WorkspaceState>>,
    Json(req): Json<CreateNoteRequest>,
) -> Json<NoteResponse> {
    debug!(workspace_id = %req.workspace_id, title = %req.title, "creating note");
    let tags = req.tags.unwrap_or_default();

    match store::create_note(
        &state.config.store_path,
        &state.config.vault_path,
        &req.workspace_id,
        &req.title,
        &req.body_markdown,
        tags,
        req.project_id.clone(),
    ) {
        Ok(note) => {
            info!(note_id = %note.note_id, "note created");
            Json(NoteResponse {
                status: "ok".to_string(),
                note,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to create note");
            let empty = make_empty_note(&req.workspace_id);
            Json(NoteResponse {
                status: "error".to_string(),
                note: empty,
            })
        }
    }
}

/// Get a single note by ID.
#[instrument(skip(state))]
pub async fn get_note(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
) -> Json<NoteResponse> {
    let workspace_id = "default";
    debug!(note_id = %id, "getting note");

    match store::get_note(
        &state.config.store_path,
        &state.config.vault_path,
        workspace_id,
        &id,
    ) {
        Ok(Some(note)) => {
            info!(note_id = %id, "note found");
            Json(NoteResponse {
                status: "ok".to_string(),
                note,
            })
        }
        Ok(None) => {
            warn!(note_id = %id, "note not found");
            let empty = make_empty_note(workspace_id);
            Json(NoteResponse {
                status: "not_found".to_string(),
                note: empty,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to get note");
            let empty = make_empty_note(workspace_id);
            Json(NoteResponse {
                status: "error".to_string(),
                note: empty,
            })
        }
    }
}

/// Update an existing note.
#[instrument(skip(state, req))]
pub async fn update_note(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNoteRequest>,
) -> Json<NoteResponse> {
    debug!(note_id = %id, "updating note");
    let tags = req.tags.unwrap_or_default();

    match store::update_note(
        &state.config.store_path,
        &state.config.vault_path,
        &id,
        &req.title,
        &req.body_markdown,
        tags,
        req.project_id.clone(),
    ) {
        Ok(note) => {
            info!(note_id = %id, "note updated");
            Json(NoteResponse {
                status: "ok".to_string(),
                note,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to update note");
            let empty = make_empty_note(&req.workspace_id);
            Json(NoteResponse {
                status: "error".to_string(),
                note: empty,
            })
        }
    }
}

/// Get Git revision history for a note.
#[instrument(skip(state))]
pub async fn note_history(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
) -> Json<NoteHistoryResponse> {
    let workspace_id = "default";
    debug!(note_id = %id, "fetching note history");

    match store::note_history(&state.config.vault_path, workspace_id, &id) {
        Ok(items) => {
            info!(note_id = %id, count = items.len(), "history fetched");
            Json(NoteHistoryResponse {
                status: "ok".to_string(),
                items,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to fetch history");
            Json(NoteHistoryResponse {
                status: "error".to_string(),
                items: vec![],
            })
        }
    }
}

/// Revert a note to a specific Git revision.
#[instrument(skip(state, req))]
pub async fn note_revert(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
    Json(req): Json<RevertNoteRequest>,
) -> Json<NoteResponse> {
    debug!(note_id = %id, revision = %req.revision, "reverting note");

    match store::revert_note(
        &state.config.store_path,
        &state.config.vault_path,
        &req.workspace_id,
        &id,
        &req.revision,
    ) {
        Ok(note) => {
            info!(note_id = %id, "note reverted");
            Json(NoteResponse {
                status: "ok".to_string(),
                note,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to revert note");
            let empty = make_empty_note(&req.workspace_id);
            Json(NoteResponse {
                status: "error".to_string(),
                note: empty,
            })
        }
    }
}

/// Get a lightweight picker list of notes (id + title only).
#[instrument(skip(state))]
pub async fn note_picker(
    State(state): State<Arc<WorkspaceState>>,
) -> Json<serde_json::Value> {
    let workspace_id = "default";
    debug!("fetching note picker");

    match store::note_picker(
        &state.config.store_path,
        &state.config.vault_path,
        workspace_id,
    ) {
        Ok(items) => {
            info!(count = items.len(), "picker items fetched");
            Json(serde_json::json!({
                "status": "ok",
                "items": items
            }))
        }
        Err(e) => {
            error!(error = %e, "failed to fetch picker");
            Json(serde_json::json!({
                "status": "error",
                "items": []
            }))
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Create an empty note for error responses.
fn make_empty_note(workspace_id: &str) -> WorkspaceNoteDocument {
    let now = chrono::Utc::now();
    WorkspaceNoteDocument {
        note_id: "error".to_string(),
        workspace_id: workspace_id.to_string(),
        project_id: None,
        slug: "error".to_string(),
        title: String::new(),
        body_markdown: String::new(),
        summary: None,
        tags: vec![],
        entity_hints: vec![],
        source: "system".to_string(),
        created_at: now,
        updated_at: now,
        archived_at: None,
        graph_status: "not_ingested".to_string(),
        markdown_path: None,
        git_revision: None,
    }
}
