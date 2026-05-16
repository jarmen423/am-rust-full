//! Graph route handlers — Phase 4: LadybugDB integration.
//!
//! All 7 handlers now query LadybugDB (where applicable) and fall back to
//! local-only data when the database is unavailable.
//!
//! Timeout: every Ladybug query is wrapped in a 5-second `tokio::time::timeout`.
//! Fallback: if LadybugDB is offline or the query times out, routes return
//! local data (notes/boards) with empty nodes/edges — never a 500 error.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, instrument, warn};

use am_workspace::model::*;
use crate::store;

use super::WorkspaceState;

// ── Query Parameters ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExploreQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    repo_id: Option<String>,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PickerQuery {
    q: Option<String>,
}

fn default_limit() -> i64 {
    50
}

// ── Helpers ──────────────────────────────────────────────────────────

fn empty_seed(seed_type: &str, seed_id: &str, title: &str) -> WorkspaceGraphSeed {
    WorkspaceGraphSeed {
        seed_type: seed_type.to_string(),
        seed_id: seed_id.to_string(),
        title: title.to_string(),
    }
}

fn graph_response(
    seed_type: &str,
    seed_id: &str,
    title: &str,
    nodes: Vec<WorkspaceGraphNode>,
    edges: Vec<WorkspaceGraphEdge>,
) -> Json<GraphResponse> {
    Json(GraphResponse {
        status: "ok".to_string(),
        seed: empty_seed(seed_type, seed_id, title),
        nodes,
        edges,
    })
}

/// Wrapper that runs a blocking LadybugDB function with a 5-second timeout.
///
/// Returns `Ok(result)` on success, `Err("timeout")` on timeout,
/// `Err(e)` on other errors.  The route handler treats all errors the same:
/// fall back to local-only data.
async fn with_lbug_timeout<T, F>(f: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    match timeout(Duration::from_secs(5), f).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => {
            warn!(error = %e, "LadybugDB query failed");
            Err(e)
        }
        Err(_) => {
            warn!("LadybugDB query timed out after 5s");
            Err("timeout".to_string())
        }
    }
}

// ── 1. Explore graph ─────────────────────────────────────────────────

/// Explore the full workspace graph (random sample from LadybugDB).
#[instrument(skip(state))]
pub async fn graph_explore(
    State(state): State<Arc<WorkspaceState>>,
    Query(params): Query<ExploreQuery>,
) -> Json<GraphResponse> {
    info!(limit = params.limit, "graph explore");

    let lbug_result = if let Some(ref conn) = state.ladybug_db {
        let conn_ref = conn.clone();
        let limit = params.limit;
        let repo_id = params.repo_id.clone();
        let project_id = params.project_id.clone();
        with_lbug_timeout(async move {
            tokio::task::spawn_blocking(move || {
                store::ladybug::explore_graph(
                    &conn_ref,
                    limit,
                    repo_id.as_deref(),
                    project_id.as_deref(),
                )
            })
            .await
            .map_err(|e| format!("join error: {}", e))?
        })
        .await
    } else {
        Err("LadybugDB not available".to_string())
    };

    match lbug_result {
        Ok((nodes, edges)) => {
            info!(node_count = nodes.len(), edge_count = edges.len(), "graph explore returned");
            graph_response("explore", "root", "Graph Explorer", nodes, edges)
        }
        Err(_) => {
            // Fallback: empty graph
            graph_response("explore", "root", "Graph Explorer", vec![], vec![])
        }
    }
}

// ── 2. Board graph ───────────────────────────────────────────────────

/// Get the subgraph rooted at a specific board.
///
/// Local structure (filesystem):
/// - Board node, object nodes, note nodes
/// - BOARD_HAS_OBJECT, OBJECT_REFERENCES_NOTE, USER_LINKED edges
///
/// Enriched with LadybugDB entities matching board title keywords.
#[instrument(skip(state))]
pub async fn graph_board(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
) -> Json<GraphResponse> {
    info!(board_id = %id, "graph board");

    // Step 1: Load board from filesystem
    let board_result = tokio::task::spawn_blocking({
        let store_path = state.config.store_path.clone();
        let board_id = id.clone();
        move || store::get_board(&store_path, "default", &board_id)
    })
    .await;

    let (mut nodes, mut edges, board_title) = match board_result {
        Ok(Ok(Some(board))) => {
            let title = board.title.clone();
            let (n, e) = store::ladybug::build_board_local_graph(
                &id,
                &board.title,
                &board.objects,
                &board.connectors,
            );
            (n, e, title)
        }
        Ok(Ok(None)) => {
            warn!(board_id = %id, "board not found");
            return graph_response("board", &id, "Unknown Board", vec![], vec![]);
        }
        Ok(Err(e)) => {
            error!(error = %e, "failed to load board");
            return graph_response("board", &id, "Error", vec![], vec![]);
        }
        Err(e) => {
            error!(error = %e, "task join error loading board");
            return graph_response("board", &id, "Error", vec![], vec![]);
        }
    };

    // Step 2: Enrich with Ladybug entities from board title keywords
    if let Some(ref conn) = state.ladybug_db {
        let keywords = store::ladybug::extract_keywords(&board_title, 3);
        if !keywords.is_empty() {
            let conn_ref = conn.clone();
            let kw = keywords.clone();
            let enrich_result = with_lbug_timeout(async move {
                tokio::task::spawn_blocking(move || {
                    let mut entity_names: Vec<String> = Vec::new();
                    for keyword in &kw {
                        match store::ladybug::search_entities_by_keyword(&conn_ref, keyword, 10) {
                            Ok(names) => {
                                for name in names {
                                    if !entity_names.contains(&name) {
                                        entity_names.push(name);
                                    }
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                    if entity_names.is_empty() {
                        return Ok((Vec::new(), Vec::new()));
                    }
                    store::ladybug::fetch_entity_relations(&conn_ref, &entity_names, 20)
                })
                .await
                .map_err(|e| format!("join error: {}", e))?
            })
            .await;

            match enrich_result {
                Ok((lb_nodes, lb_edges)) => {
                    // Merge Ladybug nodes (avoid duplicates)
                    for lb_node in lb_nodes {
                        if !nodes.iter().any(|n| n.node_id == lb_node.node_id) {
                            nodes.push(lb_node);
                        }
                    }
                    // Merge Ladybug edges (avoid duplicates)
                    for lb_edge in lb_edges {
                        if !edges.iter().any(|e| e.edge_id == lb_edge.edge_id) {
                            edges.push(lb_edge);
                        }
                    }
                }
                Err(_) => {
                    // Ladybug enrichment failed, keep local-only data
                }
            }
        }
    }

    graph_response("board", &id, &board_title, nodes, edges)
}

// ── 3. Note graph ────────────────────────────────────────────────────

/// Get the subgraph rooted at a specific note.
///
/// Local structure (filesystem):
/// - Note node as seed
/// - Boards containing this note
/// - Board nodes, object nodes
/// - BOARD_HAS_OBJECT, OBJECT_REFERENCES_NOTE edges
///
/// Enriched with LadybugDB entities matching note title keywords.
#[instrument(skip(state))]
pub async fn graph_note(
    State(state): State<Arc<WorkspaceState>>,
    Path(id): Path<String>,
) -> Json<GraphResponse> {
    info!(note_id = %id, "graph note");

    // Step 1: Load note from filesystem
    let note_result = tokio::task::spawn_blocking({
        let store_path = state.config.store_path.clone();
        let vault_path = state.config.vault_path.clone();
        let note_id = id.clone();
        move || store::get_note(&store_path, &vault_path, "default", &note_id)
    })
    .await;

    let (note_title, note_summary) = match note_result {
        Ok(Ok(Some(note))) => (note.title.clone(), note.summary.clone()),
        Ok(Ok(None)) => {
            warn!(note_id = %id, "note not found");
            return graph_response("note", &id, "Unknown Note", vec![], vec![]);
        }
        Ok(Err(e)) => {
            error!(error = %e, "failed to load note");
            return graph_response("note", &id, "Error", vec![], vec![]);
        }
        Err(e) => {
            error!(error = %e, "task join error loading note");
            return graph_response("note", &id, "Error", vec![], vec![]);
        }
    };

    // Step 2: Find all boards containing this note_id in their objects
    let boards_result = tokio::task::spawn_blocking({
        let store_path = state.config.store_path.clone();
        move || store::list_boards(&store_path, "default")
    })
    .await;

    let boards = match boards_result {
        Ok(Ok(boards)) => boards,
        _ => Vec::new(),
    };

    // Step 3: Build local graph
    let (mut nodes, mut edges) =
        store::ladybug::build_note_local_graph(&id, &note_title, &boards);

    // Step 4: Enrich with Ladybug entities from note title + summary keywords
    if let Some(ref conn) = state.ladybug_db {
        let mut keyword_source = note_title.clone();
        if let Some(ref summary) = note_summary {
            keyword_source.push(' ');
            keyword_source.push_str(summary);
        }
        let keywords = store::ladybug::extract_keywords(&keyword_source, 3);
        if !keywords.is_empty() {
            let conn_ref = conn.clone();
            let kw = keywords.clone();
            let enrich_result = with_lbug_timeout(async move {
                tokio::task::spawn_blocking(move || {
                    let mut entity_names: Vec<String> = Vec::new();
                    for keyword in &kw {
                        match store::ladybug::search_entities_by_keyword(&conn_ref, keyword, 10) {
                            Ok(names) => {
                                for name in names {
                                    if !entity_names.contains(&name) {
                                        entity_names.push(name);
                                    }
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                    if entity_names.is_empty() {
                        return Ok((Vec::new(), Vec::new()));
                    }
                    store::ladybug::fetch_entity_relations(&conn_ref, &entity_names, 20)
                })
                .await
                .map_err(|e| format!("join error: {}", e))?
            })
            .await;

            match enrich_result {
                Ok((lb_nodes, lb_edges)) => {
                    for lb_node in lb_nodes {
                        if !nodes.iter().any(|n| n.node_id == lb_node.node_id) {
                            nodes.push(lb_node);
                        }
                    }
                    for lb_edge in lb_edges {
                        if !edges.iter().any(|e| e.edge_id == lb_edge.edge_id) {
                            edges.push(lb_edge);
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    graph_response("note", &id, &note_title, nodes, edges)
}

// ── 4. Entity graph ──────────────────────────────────────────────────

/// Get the subgraph for a named entity (concept search via LadybugDB).
///
/// Primary path: search LadybugDB MemoryEntity nodes by name.
/// Fallback: local note/ingest search (same as graph_picker logic).
#[instrument(skip(state))]
pub async fn graph_entity(
    State(state): State<Arc<WorkspaceState>>,
    Path(name): Path<String>,
) -> Json<GraphResponse> {
    info!(entity_name = %name, "graph entity");

    // Try LadybugDB first
    if let Some(ref conn) = state.ladybug_db {
        let conn_ref = conn.clone();
        let entity_name = name.clone();
        let lbug_result = with_lbug_timeout(async move {
            tokio::task::spawn_blocking(move || {
                // Step 1: Find entities matching name
                let entity_names =
                    store::ladybug::search_entities_by_name(&conn_ref, &entity_name, 50)?;
                if entity_names.is_empty() {
                    return Ok((Vec::new(), Vec::new()));
                }
                // Step 2: Fetch relations
                store::ladybug::fetch_entity_relations(&conn_ref, &entity_names, 50)
            })
            .await
            .map_err(|e| format!("join error: {}", e))?
        })
        .await;

        match lbug_result {
            Ok((nodes, edges)) if !nodes.is_empty() => {
                return graph_response("entity", &name, &name, nodes, edges);
            }
            _ => {
                debug!("Ladybug entity search returned empty, trying local fallback");
            }
        }
    }

    // Fallback: local search (same as graph_picker)
    let search_name = name.to_lowercase();
    let local_result = tokio::task::spawn_blocking({
        let store_path = state.config.store_path.clone();
        let vault_path = state.config.vault_path.clone();
        let search = search_name.clone();
        move || local_entity_fallback(&store_path, &vault_path, &search)
    })
    .await;

    let (nodes, edges) = match local_result {
        Ok(Ok(result)) => result,
        _ => (Vec::new(), Vec::new()),
    };

    graph_response("entity", &name, &name, nodes, edges)
}

/// Fallback for entity_graph: search local notes by title/summary/tags.
fn local_entity_fallback(
    store_path: &str,
    vault_path: &str,
    search: &str,
) -> Result<(Vec<WorkspaceGraphNode>, Vec<WorkspaceGraphEdge>), String> {
    let mut nodes: Vec<WorkspaceGraphNode> = Vec::new();

    // Search notes
    let notes = store::list_notes(store_path, vault_path, "default")?;
    for note in notes {
        let haystack = format!(
            "{} {} {}",
            note.title.to_lowercase(),
            note.summary.as_deref().unwrap_or("").to_lowercase(),
            note.tags.join(" ").to_lowercase()
        );
        if haystack.contains(search) {
            nodes.push(WorkspaceGraphNode {
                node_id: format!("note:{}", note.note_id),
                node_type: "note".to_string(),
                title: note.title,
                subtitle: note.summary,
                metadata: serde_json::json!({
                    "source": "local",
                    "tags": note.tags,
                    "graph_status": note.graph_status,
                }),
            });
        }
    }

    // Search ingests (by scanning ingest directory)
    let ingests_path = store::vault::ingests_dir(store_path, "default");
    if ingests_path.exists() {
        for entry in std::fs::read_dir(&ingests_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
                if let Ok(ingest) =
                    serde_json::from_str::<WorkspaceIngestPayload>(&content)
                {
                    let haystack = format!(
                        "{} {} {}",
                        ingest.title.to_lowercase(),
                        ingest.summary.to_lowercase(),
                        ingest.tags.join(" ").to_lowercase()
                    );
                    if haystack.contains(search) {
                        nodes.push(WorkspaceGraphNode {
                            node_id: format!("artifact:{}", ingest.ingest_id),
                            node_type: "ingest".to_string(),
                            title: ingest.title,
                            subtitle: Some(ingest.summary),
                            metadata: serde_json::json!({
                                "source": "local",
                                "board_id": ingest.board_id,
                                "scope": ingest.ingest_scope,
                            }),
                        });
                    }
                }
            }
        }
    }

    // Limit to 20 results
    nodes.truncate(20);

    Ok((nodes, Vec::new()))
}

// ── 5. Graph picker (local search only) ──────────────────────────────

/// Search notes and ingests by query string. No LadybugDB query.
#[instrument(skip(state))]
pub async fn graph_picker(
    State(state): State<Arc<WorkspaceState>>,
    Query(params): Query<PickerQuery>,
) -> Json<serde_json::Value> {
    let query = params.q.unwrap_or_default().to_lowercase();
    info!(query = %query, "graph picker");

    let store_path = state.config.store_path.clone();
    let vault_path = state.config.vault_path.clone();
    let search = query.clone();

    let result = tokio::task::spawn_blocking(move || picker_search(&store_path, &vault_path, &search))
        .await;

    match result {
        Ok(Ok(items)) => {
            info!(count = items.len(), "picker results");
            Json(serde_json::json!({
                "status": "ok",
                "items": items
            }))
        }
        Ok(Err(e)) => {
            error!(error = %e, "picker search failed");
            Json(serde_json::json!({
                "status": "error",
                "items": []
            }))
        }
        Err(e) => {
            error!(error = %e, "picker join error");
            Json(serde_json::json!({
                "status": "error",
                "items": []
            }))
        }
    }
}

/// Local-only search for graph_picker.
fn picker_search(
    store_path: &str,
    vault_path: &str,
    search: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut items: Vec<serde_json::Value> = Vec::new();

    // Search notes
    let notes = store::list_notes(store_path, vault_path, "default")?;
    for note in notes {
        let haystack = format!(
            "{} {} {}",
            note.title.to_lowercase(),
            note.summary.as_deref().unwrap_or("").to_lowercase(),
            note.tags.join(" ").to_lowercase()
        );
        if search.is_empty() || haystack.contains(search) {
            items.push(serde_json::json!({
                "node_id": format!("note:{}", note.note_id),
                "node_type": "note",
                "title": note.title,
                "subtitle": note.summary,
                "source": "local",
            }));
        }
    }

    // Search ingests
    let ingests_path = store::vault::ingests_dir(store_path, "default");
    if ingests_path.exists() {
        for entry in std::fs::read_dir(&ingests_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
                if let Ok(ingest) =
                    serde_json::from_str::<WorkspaceIngestPayload>(&content)
                {
                    let haystack = format!(
                        "{} {} {}",
                        ingest.title.to_lowercase(),
                        ingest.summary.to_lowercase(),
                        ingest.tags.join(" ").to_lowercase()
                    );
                    if search.is_empty() || haystack.contains(search) {
                        items.push(serde_json::json!({
                            "node_id": format!("artifact:{}", ingest.ingest_id),
                            "node_type": "ingest",
                            "title": ingest.title,
                            "subtitle": ingest.summary,
                            "source": "local",
                        }));
                    }
                }
            }
        }
    }

    // Limit to 20
    items.truncate(20);
    Ok(items)
}

// ── 6. List repos ────────────────────────────────────────────────────

/// List repositories from LadybugDB.
#[instrument(skip(state))]
pub async fn graph_repos(
    State(state): State<Arc<WorkspaceState>>,
) -> Json<serde_json::Value> {
    info!("graph repos");

    let lbug_result = if let Some(ref conn) = state.ladybug_db {
        let conn_ref = conn.clone();
        with_lbug_timeout(async move {
            tokio::task::spawn_blocking(move || store::ladybug::list_repos(&conn_ref, 50))
                .await
                .map_err(|e| format!("join error: {}", e))?
        })
        .await
    } else {
        Err("LadybugDB not available".to_string())
    };

    match lbug_result {
        Ok(repos) => {
            let items: Vec<serde_json::Value> = repos
                .into_iter()
                .map(|(repo_id, count)| {
                    serde_json::json!({
                        "repo_id": repo_id,
                        "count": count,
                    })
                })
                .collect();
            Json(serde_json::json!({
                "status": "ok",
                "repos": items
            }))
        }
        Err(_) => {
            Json(serde_json::json!({
                "status": "ok",
                "repos": []
            }))
        }
    }
}

// ── 7. List projects ─────────────────────────────────────────────────

/// List projects from LadybugDB.
#[instrument(skip(state))]
pub async fn graph_projects(
    State(state): State<Arc<WorkspaceState>>,
) -> Json<serde_json::Value> {
    info!("graph projects");

    let lbug_result = if let Some(ref conn) = state.ladybug_db {
        let conn_ref = conn.clone();
        with_lbug_timeout(async move {
            tokio::task::spawn_blocking(move || store::ladybug::list_projects(&conn_ref, 50))
                .await
                .map_err(|e| format!("join error: {}", e))?
        })
        .await
    } else {
        Err("LadybugDB not available".to_string())
    };

    match lbug_result {
        Ok(projects) => {
            let items: Vec<serde_json::Value> = projects
                .into_iter()
                .map(|(project_id, count)| {
                    serde_json::json!({
                        "project_id": project_id,
                        "count": count,
                    })
                })
                .collect();
            Json(serde_json::json!({
                "status": "ok",
                "projects": items
            }))
        }
        Err(_) => {
            Json(serde_json::json!({
                "status": "ok",
                "projects": []
            }))
        }
    }
}
