//! HTTP API client for the workspace REST API.
//!
//! Uses `ehttp` for async requests that work in both native and WASM.
//! All functions return a `Promise<T>` that the UI polls each frame.

use am_workspace::model::*;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use std::sync::Arc;

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
    let mut request = ehttp::Request::new(path, body_json);
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
        project_id: None,
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
        project_id: None,
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

/// Save (update) an existing board.
/// The board's tldraw_document is serialized from the canvas document.
pub fn save_board(
    board_id: &str,
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
        workspace_id: "default".to_string(),
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
