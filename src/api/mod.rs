//! HTTP API client for the workspace REST API.
//!
//! Uses `ehttp` for async requests that work in both native and WASM.
//! All functions return a `Promise<T>` that the UI polls each frame.

use crate::model::*;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Graph/repo scope filters shared by sidebar and graph explorer.
#[derive(Debug, Clone, Default)]
pub struct ApiScope {
    pub repo_id: Option<String>,
    pub project_id: Option<String>,
}

impl ApiScope {
    fn query_suffix(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref r) = self.repo_id {
            if !r.is_empty() {
                parts.push(format!("repo_id={}", urlencoding(r)));
            }
        }
        if let Some(ref p) = self.project_id {
            if !p.is_empty() {
                parts.push(format!("project_id={}", urlencoding(p)));
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }
}

/// Async result wrapper for ehttp callbacks.
#[derive(Debug, Clone)]
pub enum Promise<T> {
    /// Nothing started yet.
    Idle,
    /// Request in flight.
    Pending,
    /// Response received and parsed.
    Ready(T),
    /// Request or parse failed.
    Failed(String),
}

impl<T> Promise<T> {
    /// Take the value if Ready, leaving Idle.
    pub fn take(&mut self) -> Option<T> {
        let mut tmp = Promise::Idle;
        std::mem::swap(self, &mut tmp);
        match tmp {
            Promise::Ready(v) => Some(v),
            other => {
                *self = other;
                None
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Promise::Pending)
    }
    pub fn is_ready(&self) -> bool {
        matches!(self, Promise::Ready(_))
    }
    pub fn is_failed(&self) -> bool {
        matches!(self, Promise::Failed(_))
    }
}

impl<T> Default for Promise<T> {
    fn default() -> Self {
        Promise::Idle
    }
}

/// Shared promise state — used to communicate from ehttp callback to UI frame.
pub type SharedPromise<T> = Arc<Mutex<Promise<T>>>;

// ── Internal: fetch helpers that unwrap server response wrappers ────

/// Response wrapper for note list: `{ status, notes }`.
#[derive(serde::Deserialize)]
struct NoteListResp {
    #[allow(dead_code)]
    status: String,
    notes: Vec<WorkspaceNoteDocument>,
}

/// Response wrapper for single note: `{ status, note }`.
#[derive(serde::Deserialize)]
struct NoteResp {
    #[allow(dead_code)]
    status: String,
    note: WorkspaceNoteDocument,
}

/// Response wrapper for note history: `{ status, items }`.
#[derive(serde::Deserialize)]
struct HistoryResp {
    #[allow(dead_code)]
    status: String,
    items: Vec<NoteHistoryItem>,
}

/// Response wrapper for board list: `{ status, boards }`.
#[derive(serde::Deserialize)]
struct BoardListResp {
    #[allow(dead_code)]
    status: String,
    boards: Vec<WorkspaceBoard>,
}

/// Response wrapper for single board document: `{ status, board, objects, connectors }`.
#[derive(serde::Deserialize)]
struct BoardDocResp {
    #[allow(dead_code)]
    status: String,
    board: WorkspaceBoard,
    #[allow(dead_code)]
    objects: Vec<crate::model::WorkspaceBoardObject>,
    #[allow(dead_code)]
    connectors: Vec<crate::model::WorkspaceConnector>,
}

/// Response wrapper for board mutation: `{ status, board }`.
#[derive(serde::Deserialize)]
struct BoardResp {
    #[allow(dead_code)]
    status: String,
    board: WorkspaceBoard,
}

/// Perform a GET request and extract data from a wrapper response.
fn get_wrapped<T, W>(
    path: &str,
    state: SharedPromise<T>,
    ctx: &egui::Context,
    extract: fn(W) -> T,
) where
    T: Send + 'static,
    W: DeserializeOwned + Send + 'static,
{
    *state.lock() = Promise::Pending;
    let request = ehttp::Request::get(path);
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<W>(&response.bytes) {
                    Ok(wrapped) => Promise::Ready(extract(wrapped)),
                    Err(e) => Promise::Failed(format!("JSON parse: {e}")),
                }
            }
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Perform a POST request and extract data from a wrapper response.
fn post_wrapped<B, T, W>(
    path: &str,
    body: &B,
    state: SharedPromise<T>,
    ctx: &egui::Context,
    extract: fn(W) -> T,
) where
    B: serde::Serialize,
    T: Send + 'static,
    W: DeserializeOwned + Send + 'static,
{
    *state.lock() = Promise::Pending;
    let body_json = match serde_json::to_vec(body) {
        Ok(v) => v,
        Err(e) => {
            *state.lock() = Promise::Failed(format!("serialize: {e}"));
            return;
        }
    };
    let mut request = ehttp::Request::post(path, body_json);
    request
        .headers
        .insert("Content-Type".to_string(), "application/json".to_string());
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<W>(&response.bytes) {
                    Ok(wrapped) => Promise::Ready(extract(wrapped)),
                    Err(e) => Promise::Failed(format!("JSON parse: {e}")),
                }
            }
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Perform a PUT request and extract data from a wrapper response.
fn put_wrapped<B, T, W>(
    path: &str,
    body: &B,
    state: SharedPromise<T>,
    ctx: &egui::Context,
    extract: fn(W) -> T,
) where
    B: serde::Serialize,
    T: Send + 'static,
    W: DeserializeOwned + Send + 'static,
{
    *state.lock() = Promise::Pending;
    let body_json = match serde_json::to_vec(body) {
        Ok(v) => v,
        Err(e) => {
            *state.lock() = Promise::Failed(format!("serialize: {e}"));
            return;
        }
    };
    let mut request = ehttp::Request::post(path, body_json);
    request.method = "PUT".to_string();
    request
        .headers
        .insert("Content-Type".to_string(), "application/json".to_string());
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<W>(&response.bytes) {
                    Ok(wrapped) => Promise::Ready(extract(wrapped)),
                    Err(e) => Promise::Failed(format!("JSON parse: {e}")),
                }
            }
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

// ── Note API ───────────────────────────────────────────────────────

/// Fetch the full note list.
pub fn fetch_notes(state: SharedPromise<Vec<WorkspaceNoteDocument>>, ctx: &egui::Context) {
    get_wrapped(
        "/api/workspace/notes",
        state,
        ctx,
        |resp: NoteListResp| resp.notes,
    );
}

/// Fetch a single note by ID.
pub fn fetch_note(
    note_id: &str,
    state: SharedPromise<WorkspaceNoteDocument>,
    ctx: &egui::Context,
) {
    get_wrapped(
        &format!("/api/workspace/notes/{note_id}"),
        state,
        ctx,
        |resp: NoteResp| resp.note,
    );
}

/// Create a new note.
pub fn create_note(
    title: &str,
    body_markdown: &str,
    tags: Vec<String>,
    project_id: Option<String>,
    state: SharedPromise<WorkspaceNoteDocument>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
        title: String,
        body_markdown: String,
        tags: Vec<String>,
        project_id: Option<String>,
    }
    let body = Body {
        workspace_id: "default".to_string(),
        title: title.to_string(),
        body_markdown: body_markdown.to_string(),
        tags,
        project_id,
    };
    post_wrapped(
        "/api/workspace/notes",
        &body,
        state,
        ctx,
        |resp: NoteResp| resp.note,
    );
}

/// Update an existing note.
pub fn update_note(
    note_id: &str,
    title: &str,
    body_markdown: &str,
    tags: Vec<String>,
    project_id: Option<String>,
    state: SharedPromise<WorkspaceNoteDocument>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
        title: String,
        body_markdown: String,
        tags: Vec<String>,
        project_id: Option<String>,
    }
    let body = Body {
        workspace_id: "default".to_string(),
        title: title.to_string(),
        body_markdown: body_markdown.to_string(),
        tags,
        project_id,
    };
    put_wrapped(
        &format!("/api/workspace/notes/{note_id}"),
        &body,
        state,
        ctx,
        |resp: NoteResp| resp.note,
    );
}

/// Fetch git history for a note.
pub fn fetch_note_history(
    note_id: &str,
    state: SharedPromise<Vec<NoteHistoryItem>>,
    ctx: &egui::Context,
) {
    get_wrapped(
        &format!("/api/workspace/notes/{note_id}/history"),
        state,
        ctx,
        |resp: HistoryResp| resp.items,
    );
}

/// Revert a note to a specific git revision.
pub fn revert_note(
    note_id: &str,
    revision: &str,
    state: SharedPromise<WorkspaceNoteDocument>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
        revision: String,
    }
    let body = Body {
        workspace_id: "default".to_string(),
        revision: revision.to_string(),
    };
    post_wrapped(
        &format!("/api/workspace/notes/{note_id}/revert"),
        &body,
        state,
        ctx,
        |resp: NoteResp| resp.note,
    );
}

// ── Board API ──────────────────────────────────────────────────────

/// Fetch the board list.
pub fn fetch_boards(state: SharedPromise<Vec<WorkspaceBoard>>, ctx: &egui::Context) {
    get_wrapped(
        "/api/workspace/boards",
        state,
        ctx,
        |resp: BoardListResp| resp.boards,
    );
}

/// Fetch a single board by ID (includes document, objects, connectors).
pub fn fetch_board(
    board_id: &str,
    state: SharedPromise<WorkspaceBoard>,
    ctx: &egui::Context,
) {
    get_wrapped(
        &format!("/api/workspace/boards/{board_id}"),
        state,
        ctx,
        |resp: BoardDocResp| resp.board,
    );
}

/// Delete a board by id.
pub fn delete_board(board_id: &str, state: SharedPromise<bool>, ctx: &egui::Context) {
    *state.lock() = Promise::Pending;
    let path = format!("/api/workspace/boards/{board_id}");
    let mut request = ehttp::Request::post(&path, vec![]);
    request.method = "DELETE".to_string();
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => Promise::Ready(true),
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Save (update) an existing board.
/// The board's tldraw_document is serialized from the canvas document.
pub fn save_board(
    board_id: &str,
    workspace_id: &str,
    title: &str,
    tldraw_document: &serde_json::Value,
    objects: Vec<crate::model::WorkspaceBoardObject>,
    connectors: Vec<crate::model::WorkspaceConnector>,
    state: SharedPromise<WorkspaceBoard>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
        title: String,
        tldraw_document: serde_json::Value,
        objects: Vec<crate::model::WorkspaceBoardObject>,
        connectors: Vec<crate::model::WorkspaceConnector>,
        description: Option<String>,
        tags: Option<Vec<String>>,
        board_state: Option<String>,
    }
    let body = Body {
        workspace_id: workspace_id.to_string(),
        title: title.to_string(),
        tldraw_document: tldraw_document.clone(),
        objects,
        connectors,
        description: None,
        tags: None,
        board_state: None,
    };
    put_wrapped(
        &format!("/api/workspace/boards/{board_id}"),
        &body,
        state,
        ctx,
        |resp: BoardResp| resp.board,
    );
}

/// Create a new board with a custom document envelope.
pub fn create_board_with_document(
    workspace_id: &str,
    title: &str,
    tldraw_document: serde_json::Value,
    state: SharedPromise<WorkspaceBoard>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
        title: String,
        board_type: Option<String>,
        tldraw_document: Option<serde_json::Value>,
    }
    let body = Body {
        workspace_id: workspace_id.to_string(),
        title: title.to_string(),
        board_type: Some("canvas".to_string()),
        tldraw_document: Some(tldraw_document),
    };
    post_wrapped(
        "/api/workspace/boards",
        &body,
        state,
        ctx,
        |resp: BoardResp| resp.board,
    );
}

/// Create a new board.
pub fn create_board(
    workspace_id: &str,
    title: &str,
    state: SharedPromise<WorkspaceBoard>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
        title: String,
        board_type: Option<String>,
        tldraw_document: Option<serde_json::Value>,
    }
    let body = Body {
        workspace_id: workspace_id.to_string(),
        title: title.to_string(),
        board_type: Some("canvas".to_string()),
        tldraw_document: Some(serde_json::json!({
            "engine": "agentic_canvas",
            "version": 1,
            "camera": { "x": 0.0, "y": 0.0, "zoom": 1.0 },
            "objects": {},
            "connectors": {}
        })),
    };
    post_wrapped(
        "/api/workspace/boards",
        &body,
        state,
        ctx,
        |resp: BoardResp| resp.board,
    );
}

// ── Graph API ──────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct GraphExploreResp {
    #[allow(dead_code)]
    status: String,
    seed: crate::model::WorkspaceGraphSeed,
    nodes: Vec<crate::model::WorkspaceGraphNode>,
    edges: Vec<crate::model::WorkspaceGraphEdge>,
}

/// Fetch the workspace explore graph.
pub fn fetch_graph_explore(
    scope: &ApiScope,
    state: SharedPromise<crate::model::GraphResponse>,
    ctx: &egui::Context,
) {
    get_wrapped(
        &format!("/api/workspace/graph/explore{}", scope.query_suffix()),
        state,
        ctx,
        |resp: GraphExploreResp| crate::model::GraphResponse {
            status: resp.status,
            seed: resp.seed,
            nodes: resp.nodes,
            edges: resp.edges,
        },
    );
}

/// Fetch the subgraph for a note.
pub fn fetch_graph_note(
    note_id: &str,
    state: SharedPromise<crate::model::GraphResponse>,
    ctx: &egui::Context,
) {
    get_wrapped(
        &format!("/api/workspace/graph/note/{note_id}"),
        state,
        ctx,
        |resp: GraphExploreResp| crate::model::GraphResponse {
            status: resp.status,
            seed: resp.seed,
            nodes: resp.nodes,
            edges: resp.edges,
        },
    );
}

/// Fetch the subgraph for a board.
pub fn fetch_graph_board(
    board_id: &str,
    state: SharedPromise<crate::model::GraphResponse>,
    ctx: &egui::Context,
) {
    get_wrapped(
        &format!("/api/workspace/graph/board/{board_id}"),
        state,
        ctx,
        |resp: GraphExploreResp| crate::model::GraphResponse {
            status: resp.status,
            seed: resp.seed,
            nodes: resp.nodes,
            edges: resp.edges,
        },
    );
}

/// Search graph picker items.
pub fn fetch_graph_picker(
    query: &str,
    state: SharedPromise<Vec<serde_json::Value>>,
    ctx: &egui::Context,
) {
    let path = if query.is_empty() {
        "/api/workspace/graph/picker".to_string()
    } else {
        format!("/api/workspace/graph/picker?q={}", urlencoding(query))
    };
    *state.lock() = Promise::Pending;
    let request = ehttp::Request::get(&path);
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<serde_json::Value>(&response.bytes) {
                    Ok(json) => {
                        let items = json
                            .get("items")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        Promise::Ready(items)
                    }
                    Err(e) => Promise::Failed(format!("JSON parse: {e}")),
                }
            }
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Ingest full board into Ladybug graph.
pub fn ingest_board(
    board_id: &str,
    workspace_id: &str,
    state: SharedPromise<WorkspaceIngestPayload>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        workspace_id: String,
    }
    let body = Body {
        workspace_id: workspace_id.to_string(),
    };
    post_wrapped(
        &format!("/api/workspace/boards/{board_id}/ingest"),
        &body,
        state,
        ctx,
        |v: IngestResp| v.payload,
    );
}

#[derive(serde::Deserialize)]
struct IngestResp {
    #[allow(dead_code)]
    status: String,
    #[serde(alias = "ingest")]
    payload: WorkspaceIngestPayload,
}

/// Fetch diagnostics health.
pub fn fetch_diagnostics_health(
    state: SharedPromise<DiagnosticsHealthResponse>,
    ctx: &egui::Context,
) {
    *state.lock() = Promise::Pending;
    let request = ehttp::Request::get("/api/workspace/diagnostics/health");
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<DiagnosticsHealthResponse>(&response.bytes) {
                    Ok(v) => Promise::Ready(v),
                    Err(e) => Promise::Failed(format!("JSON parse: {e}")),
                }
            }
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Ping diagnostics channel; returns attempt_id.
pub fn ping_diagnostics(state: SharedPromise<String>, ctx: &egui::Context) {
    #[derive(serde::Deserialize)]
    struct PingResp {
        attempt: AttemptPing,
    }
    #[derive(serde::Deserialize)]
    struct AttemptPing {
        attempt_id: String,
    }
    *state.lock() = Promise::Pending;
    let request = ehttp::Request::post("/api/workspace/diagnostics/ping", vec![]);
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => match serde_json::from_slice::<PingResp>(&response.bytes)
            {
                Ok(p) => Promise::Ready(p.attempt.attempt_id),
                Err(e) => Promise::Failed(format!("JSON parse: {e}")),
            },
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Execute read-only Cypher.
pub fn execute_query(
    cypher: &str,
    state: SharedPromise<Result<crate::query_shell::QueryResultView, String>>,
    ctx: &egui::Context,
) {
    #[derive(serde::Serialize)]
    struct Body {
        cypher: String,
    }
    *state.lock() = Promise::Pending;
    let body = serde_json::to_vec(&Body {
        cypher: cypher.to_string(),
    })
    .unwrap_or_default();
    let mut request = ehttp::Request::post("/api/workspace/query/execute", body);
    request
        .headers
        .insert("Content-Type".to_string(), "application/json".to_string());
    let ctx = ctx.clone();
    ehttp::fetch(request, move |result| {
        *state.lock() = match result {
            Ok(response) if response.ok => match serde_json::from_slice::<serde_json::Value>(&response.bytes) {
                Ok(json) => {
                    if json.get("status").and_then(|s| s.as_str()) == Some("ok") {
                        let columns = json
                            .get("columns")
                            .and_then(|c| c.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let rows = json
                            .get("rows")
                            .and_then(|r| r.as_array())
                            .map(|outer| {
                                outer
                                    .iter()
                                    .filter_map(|row| {
                                        row.as_array().map(|cells| {
                                            cells
                                                .iter()
                                                .filter_map(|c| c.as_str().map(String::from))
                                                .collect()
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let attempt_id = json
                            .get("attempt_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Promise::Ready(Ok(crate::query_shell::QueryResultView {
                            columns,
                            rows,
                            attempt_id,
                        }))
                    } else {
                        let msg = json
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("query failed")
                            .to_string();
                        Promise::Ready(Err(msg))
                    }
                }
                Err(e) => Promise::Failed(format!("JSON parse: {e}")),
            },
            Ok(response) => Promise::Failed(format!("HTTP {}", response.status)),
            Err(e) => Promise::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Agent chat request.
pub fn agent_chat(
    message: &str,
    note_id: Option<&str>,
    board_id: Option<&str>,
    project_id: Option<String>,
    state: SharedPromise<AgentChatResponse>,
    ctx: &egui::Context,
) {
    let body = AgentChatRequest {
        workspace_id: "default".to_string(),
        project_id,
        message: message.to_string(),
        note_id: note_id.map(String::from),
        board_id: board_id.map(String::from),
    };
    post_wrapped(
        "/api/workspace/agent/chat",
        &body,
        state,
        ctx,
        |v: AgentChatWrap| v,
    );
}

type AgentChatWrap = AgentChatResponse;

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
