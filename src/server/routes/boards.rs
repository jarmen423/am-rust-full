//! Board CRUD route handlers.
//!
//! Provides: list, create, get, update, ingest_board, ingest_selection.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tracing::{debug, error, info, instrument, warn};

use am_workspace::model::*;
use crate::store;

use super::WorkspaceState;

// ── Request Bodies ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateBoardRequest {
    pub workspace_id: String,
    pub title: String,
    pub board_type: Option<String>,
    pub tldraw_document: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBoardRequest {
    pub workspace_id: String,
    pub title: String,
    pub tldraw_document: serde_json::Value,
    #[serde(default)]
    pub objects: Vec<WorkspaceBoardObject>,
    #[serde(default)]
    pub connectors: Vec<WorkspaceConnector>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub board_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IngestBoardRequest {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
pub struct IngestSelectionRequest {
    pub workspace_id: String,
    pub object_ids: Vec<String>,
    pub connector_ids: Vec<String>,
}

// ── Handlers ───────────────────────────────────────────────────────

/// List all boards for the default workspace.
#[instrument(skip(state))]
pub async fn list_boards(
    State(state): State<Arc<WorkspaceState>>,
) -> Json<BoardListResponse> {
    let workspace_id = "default";
    debug!(workspace_id, "listing boards");

    match store::list_boards(&state.config.store_path, workspace_id) {
        Ok(boards) => {
            info!(count = boards.len(), "boards listed");
            Json(BoardListResponse {
                status: "ok".to_string(),
                boards,
            })
        }
        Err(e) => {
            error!(error = %e, "failed to list boards");
            Json(BoardListResponse {
                status: "error".to_string(),
                boards: vec![],
            })
        }
    }
}

/// Create a new board.
#[instrument(skip(state, req))]
pub async fn create_board(
    State(state): State<Arc<WorkspaceState>>,
    Json(req): Json<CreateBoardRequest>,
) -> Json<serde_json::Value> {
    debug!(workspace_id = %req.workspace_id, title = %req.title, "creating board");

    // Default board_type to "canvas" if not provided
    let board_type = req.board_type.or_else(|| Some("canvas".to_string()));

    match store::create_board(
        &state.config.store_path,
        &state.config.vault_path,
        &req.workspace_id,
        &req.title,
        board_type,
        req.tldraw_document,
    ) {
        Ok(board) => {
            info!(board_id = %board.board_id, "board created");
            Json(serde_json::json!({
                "status": "ok",
                "board": board
            }))
        }
        Err(e) => {
            error!(error = %e, "failed to create board");
            Json(serde_json::json!({
                "status": "error",
                "message": e
            }))
        }
    }
}

/// Get a single board by ID (includes objects and connectors inline).
#[instrument(skip(state))]
pub async fn get_board(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
) -> Json<BoardDocumentPayload> {
    let workspace_id = "default";
    debug!(board_id = %id, "getting board");

    match store::get_board(&state.config.store_path, workspace_id, &id) {
        Ok(Some(board)) => {
            info!(board_id = %id, "board found");
            let objects = board.objects.clone();
            let connectors = board.connectors.clone();
            Json(BoardDocumentPayload {
                status: "ok".to_string(),
                board,
                objects,
                connectors,
            })
        }
        Ok(None) => {
            warn!(board_id = %id, "board not found");
            let empty_board = make_empty_board(workspace_id, &id);
            Json(BoardDocumentPayload {
                status: "not_found".to_string(),
                board: empty_board,
                objects: vec![],
                connectors: vec![],
            })
        }
        Err(e) => {
            error!(error = %e, "failed to get board");
            let empty_board = make_empty_board(workspace_id, &id);
            Json(BoardDocumentPayload {
                status: "error".to_string(),
                board: empty_board,
                objects: vec![],
                connectors: vec![],
            })
        }
    }
}

/// Update an existing board (title, tldraw document, objects, connectors).
#[instrument(skip(state, req))]
pub async fn update_board(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateBoardRequest>,
) -> Json<serde_json::Value> {
    debug!(board_id = %id, "updating board");

    match store::update_board(
        &state.config.store_path,
        &id,
        &req.title,
        req.tldraw_document,
        req.objects,
        req.connectors,
        store::BoardUpdateMeta {
            workspace_id: req.workspace_id.clone(),
            description: req.description.clone(),
            tags: req.tags.clone(),
            board_state: req.board_state.clone(),
        },
    ) {
        Ok(board) => {
            info!(board_id = %id, "board updated");
            Json(serde_json::json!({
                "status": "ok",
                "board": board
            }))
        }
        Err(e) => {
            error!(error = %e, "failed to update board");
            Json(serde_json::json!({
                "status": "error",
                "message": e
            }))
        }
    }
}

/// Delete a board from the workspace store.
#[instrument(skip(state))]
pub async fn delete_board(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let store_root = &state.config.store_path;
    let store = std::path::Path::new(store_root);
    if store.exists() {
        for entry in std::fs::read_dir(store).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
            )
        })? {
            let entry = entry.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
                )
            })?;
            let ws_id = entry.file_name().to_string_lossy().to_string();
            let json_path = crate::store::vault::board_json_path(store_root, &ws_id, &id);
            if json_path.exists() {
                store::delete_board(store_root, &ws_id, &id).map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "status": "error", "message": e })),
                    )
                })?;
                info!(board_id = %id, workspace_id = %ws_id, "board deleted");
                return Ok(Json(serde_json::json!({
                    "status": "ok",
                    "board_id": id,
                    "workspace_id": ws_id
                })));
            }
        }
    }
    Err((
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "status": "error",
            "message": format!("board not found: {}", id)
        })),
    ))
}

/// Ingest an entire board — persists an ingest receipt under the workspace vault.
#[instrument(skip(state, req))]
pub async fn ingest_board(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
    Json(req): Json<IngestBoardRequest>,
) -> Json<serde_json::Value> {
    let workspace_id = req.workspace_id.as_str();

    let title = match store::get_board(&state.config.store_path, workspace_id, &id) {
        Ok(Some(b)) => b.title,
        Ok(None) => {
            warn!(board_id = %id, "ingest: board not found");
            return Json(serde_json::json!({
                "status": "error",
                "message": "board not found"
            }));
        }
        Err(e) => {
            error!(error = %e, "ingest: failed to load board");
            return Json(serde_json::json!({
                "status": "error",
                "message": e
            }));
        }
    };

    match store::ingest_board(&state.config.store_path, workspace_id, &id, &title) {
        Ok(ingest) => Json(serde_json::json!({
            "status": "ok",
            "ingest": ingest
        })),
        Err(e) => {
            error!(error = %e, "ingest_board failed");
            Json(serde_json::json!({
                "status": "error",
                "message": e
            }))
        }
    }
}

/// Ingest a selection of objects/connectors from a board.
#[instrument(skip(state, req))]
pub async fn ingest_selection(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
    Json(req): Json<IngestSelectionRequest>,
) -> Json<serde_json::Value> {
    info!(
        board_id = %id,
        object_count = req.object_ids.len(),
        connector_count = req.connector_ids.len(),
        "ingesting selection"
    );

    let workspace_id = req.workspace_id.as_str();
    let title = format!(
        "Selection on board {} ({} objects, {} connectors)",
        id,
        req.object_ids.len(),
        req.connector_ids.len()
    );

    match store::ingest_selection(
        &state.config.store_path,
        workspace_id,
        &id,
        &title,
        req.object_ids.clone(),
        req.connector_ids.clone(),
    ) {
        Ok(ingest) => Json(serde_json::json!({
            "status": "ok",
            "ingest": ingest
        })),
        Err(e) => {
            error!(error = %e, "ingest_selection failed");
            Json(serde_json::json!({
                "status": "error",
                "message": e
            }))
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Create an empty board for error / not-found responses.
fn make_empty_board(workspace_id: &str, board_id: &str) -> WorkspaceBoard {
    let now = chrono::Utc::now();
    WorkspaceBoard {
        board_id: board_id.to_string(),
        workspace_id: workspace_id.to_string(),
        project_id: None,
        title: String::new(),
        description: None,
        tags: vec![],
        board_type: "canvas".to_string(),
        board_state: "active".to_string(),
        tldraw_document: serde_json::json!({}),
        objects: vec![],
        connectors: vec![],
        object_count: 0,
        connector_count: 0,
        created_at: now,
        updated_at: now,
        ingested_at: None,
        graph_status: "not_ingested".to_string(),
    }
}
