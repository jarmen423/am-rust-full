use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{WorkspaceBoardObject, WorkspaceConnector, WorkspaceNoteDocument};

pub const AGENTIC_CANVAS_ENGINE: &str = "agentic_canvas";
pub const EXCALIDRAW_ENGINE: &str = "excalidraw";
pub const AGENTIC_CANVAS_VERSION: i32 = 1;
pub const EXCALIDRAW_VERSION: i32 = 1;
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

/// Board objects and connectors derived from a canvas document for persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasArtifacts {
    pub objects: Vec<WorkspaceBoardObject>,
    pub connectors: Vec<WorkspaceConnector>,
}

/// Whether a board document envelope uses the Excalidraw hybrid engine.
pub fn is_excalidraw_document(value: &serde_json::Value) -> bool {
    value
        .get("engine")
        .and_then(|e| e.as_str())
        == Some(EXCALIDRAW_ENGINE)
}

/// Empty Excalidraw scene envelope for hybrid draw mode.
pub fn create_empty_excalidraw_document() -> serde_json::Value {
    serde_json::json!({
        "engine": EXCALIDRAW_ENGINE,
        "version": EXCALIDRAW_VERSION,
        "scene": { "elements": [], "appState": {} }
    })
}

/// Create an empty first-party canvas document.
pub fn create_empty_canvas_document() -> WorkspaceCanvasDocument {
    WorkspaceCanvasDocument {
        engine: AGENTIC_CANVAS_ENGINE.to_string(),
        version: AGENTIC_CANVAS_VERSION,
        camera: CanvasCamera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        },
        objects: HashMap::new(),
        connectors: HashMap::new(),
    }
}

/// Strip frontmatter and markdown noise for card previews.
pub fn build_markdown_preview(markdown: &str) -> String {
    let text = strip_frontmatter(markdown);
    let text = text
        .replace("```", " code block ")
        .chars()
        .map(|c| match c {
            '#' | '>' | '*' | '_' | '`' | '[' | ']' | '(' | ')' | '-' => ' ',
            other => other,
        })
        .collect::<String>();
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() > 180 {
        format!("{}...", &text[..177])
    } else if text.is_empty() {
        "No body yet.".to_string()
    } else {
        text
    }
}

fn strip_frontmatter(markdown: &str) -> &str {
    if markdown.starts_with("---") {
        if let Some(end) = markdown[3..].find("\n---") {
            let rest = &markdown[3 + end + 4..];
            return rest.trim_start();
        }
    }
    markdown
}

fn canvas_object_kind_str(kind: &CanvasObjectKind) -> &'static str {
    match kind {
        CanvasObjectKind::NoteCard => "note_card",
        CanvasObjectKind::TextCard => "text_card",
        CanvasObjectKind::GraphReference => "graph_reference",
    }
}

fn default_note_position(document: &WorkspaceCanvasDocument) -> (f32, f32) {
    let count = document.objects.len();
    (
        120.0 + (count % 5) as f32 * 36.0,
        120.0 + (count % 5) as f32 * 32.0,
    )
}

/// Place a note-backed card on the canvas at an optional world position.
pub fn add_note_to_canvas_document(
    document: &WorkspaceCanvasDocument,
    note: &WorkspaceNoteDocument,
    position: Option<(f32, f32)>,
) -> WorkspaceCanvasDocument {
    let (x, y) = position.unwrap_or_else(|| default_note_position(document));
    let object_id = format!(
        "canvas-note-{}-{}",
        note.note_id,
        Utc::now().timestamp_millis()
    );

    let mut objects = document.objects.clone();
    objects.insert(
        object_id.clone(),
        CanvasObject {
            id: object_id,
            kind: CanvasObjectKind::NoteCard,
            x,
            y,
            w: NOTE_CARD_WIDTH,
            h: NOTE_CARD_HEIGHT,
            note_id: Some(note.note_id.clone()),
            title: note.title.clone(),
            summary: note.summary.clone().unwrap_or_default(),
            markdown_preview: build_markdown_preview(&note.body_markdown),
            tags: note.tags.clone(),
            locked: false,
        },
    );

    WorkspaceCanvasDocument {
        objects,
        ..document.clone()
    }
}

/// Project canvas cards/connectors into board graph persistence rows.
pub fn derive_workspace_artifacts_from_canvas_document(
    document: &WorkspaceCanvasDocument,
    board_id: &str,
    workspace_id: &str,
    project_id: Option<&str>,
) -> CanvasArtifacts {
    let now = Utc::now();
    let project_id = project_id.map(|s| s.to_string());

    let objects: Vec<WorkspaceBoardObject> = document
        .objects
        .values()
        .map(|object| WorkspaceBoardObject {
            object_id: object.id.clone(),
            board_id: board_id.to_string(),
            workspace_id: workspace_id.to_string(),
            project_id: project_id.clone(),
            object_type: canvas_object_kind_str(&object.kind).to_string(),
            title: Some(object.title.clone()),
            summary: if object.summary.is_empty() {
                if object.markdown_preview.is_empty() {
                    None
                } else {
                    Some(object.markdown_preview.clone())
                }
            } else {
                Some(object.summary.clone())
            },
            note_id: object.note_id.clone(),
            asset_id: None,
            artifact_id: None,
            graph_entity_name: None,
            graph_source_id: None,
            tags: object.tags.clone(),
            ingest_eligible: true,
            locked: object.locked,
            created_at: now,
            updated_at: now,
        })
        .collect();

    let connectors: Vec<WorkspaceConnector> = document
        .connectors
        .values()
        .filter(|connector| {
            document.objects.contains_key(&connector.from_object_id)
                && document.objects.contains_key(&connector.to_object_id)
        })
        .map(|connector| WorkspaceConnector {
            connector_id: connector.id.clone(),
            board_id: board_id.to_string(),
            workspace_id: workspace_id.to_string(),
            project_id: project_id.clone(),
            from_object_id: connector.from_object_id.clone(),
            to_object_id: connector.to_object_id.clone(),
            connector_type: "directed".to_string(),
            relation_intent: if connector.relation_intent.is_empty() {
                "linked".to_string()
            } else {
                connector.relation_intent.clone()
            },
            custom_label: if connector.label.is_empty() {
                None
            } else {
                Some(connector.label.clone())
            },
            user_authored_summary: None,
            ingest_eligible: true,
            created_at: now,
            updated_at: now,
        })
        .collect();

    CanvasArtifacts { objects, connectors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_note() -> WorkspaceNoteDocument {
        WorkspaceNoteDocument {
            note_id: "note-1".to_string(),
            workspace_id: "default".to_string(),
            project_id: None,
            slug: "note-1".to_string(),
            title: "Hello Note".to_string(),
            body_markdown: "# Title\n\nSome **markdown** body.".to_string(),
            summary: Some("Short summary".to_string()),
            tags: vec!["tag-a".to_string()],
            entity_hints: vec![],
            source: "test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            archived_at: None,
            graph_status: "not_ingested".to_string(),
            markdown_path: None,
            git_revision: None,
        }
    }

    #[test]
    fn add_note_places_card_at_position() {
        let doc = create_empty_canvas_document();
        let note = sample_note();
        let updated = add_note_to_canvas_document(&doc, &note, Some((50.0, 75.0)));

        assert_eq!(updated.objects.len(), 1);
        let obj = updated.objects.values().next().unwrap();
        assert_eq!(obj.x, 50.0);
        assert_eq!(obj.y, 75.0);
        assert_eq!(obj.note_id.as_deref(), Some("note-1"));
        assert_eq!(obj.kind, CanvasObjectKind::NoteCard);
    }

    #[test]
    fn derive_artifacts_maps_objects_and_connectors() {
        let mut doc = create_empty_canvas_document();
        let note = sample_note();
        doc = add_note_to_canvas_document(&doc, &note, None);
        let object_id = doc.objects.keys().next().unwrap().clone();

        doc.connectors.insert(
            "conn-1".to_string(),
            CanvasConnector {
                id: "conn-1".to_string(),
                from_object_id: object_id.clone(),
                to_object_id: "missing".to_string(),
                relation_intent: "related".to_string(),
                label: String::new(),
            },
        );

        let artifacts =
            derive_workspace_artifacts_from_canvas_document(&doc, "board-1", "default", None);

        assert_eq!(artifacts.objects.len(), 1);
        assert_eq!(artifacts.objects[0].object_type, "note_card");
        assert_eq!(artifacts.objects[0].note_id.as_deref(), Some("note-1"));
        // Connector to missing object is filtered out
        assert!(artifacts.connectors.is_empty());
    }

    #[test]
    fn derive_artifacts_keeps_valid_connectors() {
        let mut doc = create_empty_canvas_document();
        let note_a = sample_note();
        doc = add_note_to_canvas_document(&doc, &note_a, Some((0.0, 0.0)));
        let from_id = doc.objects.keys().next().unwrap().clone();

        let mut note_b = sample_note();
        note_b.note_id = "note-2".to_string();
        note_b.title = "Second".to_string();
        doc = add_note_to_canvas_document(&doc, &note_b, Some((400.0, 0.0)));
        let to_id = doc
            .objects
            .values()
            .find(|o| o.note_id.as_deref() == Some("note-2"))
            .unwrap()
            .id
            .clone();

        doc.connectors.insert(
            "conn-1".to_string(),
            CanvasConnector {
                id: "conn-1".to_string(),
                from_object_id: from_id,
                to_object_id: to_id,
                relation_intent: "depends_on".to_string(),
                label: "uses".to_string(),
            },
        );

        let artifacts =
            derive_workspace_artifacts_from_canvas_document(&doc, "board-1", "default", None);

        assert_eq!(artifacts.objects.len(), 2);
        assert_eq!(artifacts.connectors.len(), 1);
        assert_eq!(artifacts.connectors[0].relation_intent, "depends_on");
        assert_eq!(artifacts.connectors[0].custom_label.as_deref(), Some("uses"));
    }

    #[test]
    fn build_markdown_preview_strips_noise() {
        let preview = build_markdown_preview("# Hello\n\n**world**");
        assert!(preview.contains("Hello"));
        assert!(preview.contains("world"));
        assert!(!preview.contains('#'));
    }
}
