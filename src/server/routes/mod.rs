pub mod boards;
pub mod graph;
pub mod notes;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::config::ServerConfig;

/// Shared application state passed to every handler via Axum's State extractor.
#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub config: ServerConfig,
    /// Optional LadybugDB connection.  `None` when no `.lbug` file is found
    /// or the connection could not be established.  Routes must handle this
    /// gracefully (fallback to local-only data).
    pub ladybug_db: Option<crate::store::ladybug::LadybugDb>,
}

/// Assemble all API routes into a single Router.
pub fn create_routes(state: Arc<WorkspaceState>) -> Router {
    Router::new()
        // ── Bootstrap ──────────────────────────────────────────────
        .route("/api/workspace/bootstrap", get(notes::bootstrap))
        // ── Notes ──────────────────────────────────────────────────
        .route("/api/workspace/notes", get(notes::list_notes).post(notes::create_note))
        .route("/api/workspace/notes/picker", get(notes::note_picker))
        .route("/api/workspace/notes/{id}", get(notes::get_note).put(notes::update_note))
        .route("/api/workspace/notes/{id}/history", get(notes::note_history))
        .route("/api/workspace/notes/{id}/revert", post(notes::note_revert))
        // ── Boards ─────────────────────────────────────────────────
        .route("/api/workspace/boards", get(boards::list_boards).post(boards::create_board))
        .route("/api/workspace/boards/{id}", get(boards::get_board).put(boards::update_board))
        .route("/api/workspace/boards/{id}/ingest", post(boards::ingest_board))
        .route("/api/workspace/boards/{id}/ingest-selection", post(boards::ingest_selection))
        // ── Graph (Phase 4 — LadybugDB integration) ────────────────
        .route("/api/workspace/graph/explore", get(graph::graph_explore))
        .route("/api/workspace/graph/picker", get(graph::graph_picker))
        .route("/api/workspace/graph/note/{id}", get(graph::graph_note))
        .route("/api/workspace/graph/board/{id}", get(graph::graph_board))
        .route("/api/workspace/graph/entity/{name}", get(graph::graph_entity))
        .route("/api/workspace/graph/repos", get(graph::graph_repos))
        .route("/api/workspace/graph/projects", get(graph::graph_projects))
        .with_state(state)
}
