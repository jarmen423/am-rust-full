use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const AGENTIC_CANVAS_ENGINE: &str = "agentic_canvas";
pub const AGENTIC_CANVAS_VERSION: i32 = 1;
pub const NOTE_CARD_WIDTH: f32 = 340.0;
pub const NOTE_CARD_HEIGHT: f32 = 220.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasCamera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasObjectKind {
    NoteCard,
    TextCard,
    GraphReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasObject {
    pub id: String,
    pub kind: CanvasObjectKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub markdown_preview: String,
    pub tags: Vec<String>,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasConnector {
    pub id: String,
    pub from_object_id: String,
    pub to_object_id: String,
    pub relation_intent: String,
    pub label: String,
}

/// Top-level canvas document stored inside `WorkspaceBoard.tldraw_document`.
/// Keys use camelCase in JSON to match TypeScript output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceCanvasDocument {
    pub engine: String,
    pub version: i32,
    pub camera: CanvasCamera,
    pub objects: HashMap<String, CanvasObject>,
    pub connectors: HashMap<String, CanvasConnector>,
}
