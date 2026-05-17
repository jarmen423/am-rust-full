use am_workspace::model::{
    WorkspaceBoard, WorkspaceBoardObject, WorkspaceConnector, WorkspaceIngestPayload,
};
use crate::store::vault;
use chrono::Utc;
use serde_json;
use uuid::Uuid;

/// Metadata sent with `PUT /boards/{id}` beyond title / tldraw / geometry.
#[derive(Debug, Clone, Default)]
pub struct BoardUpdateMeta {
    pub workspace_id: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub board_state: Option<String>,
}

/// Create a new board with default empty state.
pub fn create_board(
    store_root: &str,
    vault_root: &str,
    workspace_id: &str,
    title: &str,
    board_type: Option<String>,
    tldraw_document: Option<serde_json::Value>,
) -> Result<WorkspaceBoard, String> {
    let now = Utc::now();
    let board_id = Uuid::new_v4().to_string();
    let board_type = board_type.unwrap_or_else(|| "canvas".to_string());
    let tldraw_document = tldraw_document.unwrap_or_else(|| serde_json::json!({}));

    // Ensure directories exist
    vault::ensure_workspace_dirs(store_root, vault_root, workspace_id)
        .map_err(|e| format!("ensure dirs: {}", e))?;

    let board = WorkspaceBoard {
        board_id: board_id.clone(),
        workspace_id: workspace_id.to_string(),
        project_id: None,
        title: title.to_string(),
        description: None,
        tags: Vec::new(),
        board_type,
        board_state: "active".to_string(),
        tldraw_document,
        objects: Vec::new(),
        connectors: Vec::new(),
        object_count: 0,
        connector_count: 0,
        created_at: now,
        updated_at: now,
        ingested_at: None,
        graph_status: "not_ingested".to_string(),
    };

    // Persist to JSON
    let json_path = vault::board_json_path(store_root, workspace_id, &board_id);
    if let Some(parent) = json_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create boards dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&board)
        .map_err(|e| format!("serialize board: {}", e))?;
    std::fs::write(&json_path, json).map_err(|e| format!("write board: {}", e))?;

    Ok(board)
}

/// Update an existing board.
pub fn update_board(
    store_root: &str,
    board_id: &str,
    title: &str,
    tldraw_document: serde_json::Value,
    objects: Vec<WorkspaceBoardObject>,
    connectors: Vec<WorkspaceConnector>,
    meta: BoardUpdateMeta,
) -> Result<WorkspaceBoard, String> {
    // Find the board by scanning all workspace directories
    let mut board = None;
    let store = std::path::Path::new(store_root);
    if store.exists() {
        for entry in std::fs::read_dir(store).map_err(|e| format!("read store: {}", e))? {
            let entry = entry.map_err(|e| format!("store entry: {}", e))?;
            let ws_id = entry.file_name().to_string_lossy().to_string();
            let json_path = vault::board_json_path(store_root, &ws_id, board_id);
            if json_path.exists() {
                let content = std::fs::read_to_string(&json_path)
                    .map_err(|e| format!("read board: {}", e))?;
                let mut b: WorkspaceBoard = serde_json::from_str(&content)
                    .map_err(|e| format!("parse board: {}", e))?;
                b.title = title.to_string();
                b.tldraw_document = tldraw_document;
                b.objects = objects;
                b.connectors = connectors;
                b.object_count = b.objects.len() as i32;
                b.connector_count = b.connectors.len() as i32;
                if !meta.workspace_id.is_empty() && meta.workspace_id != b.workspace_id {
                    return Err(format!(
                        "workspace_id mismatch: board is in {}, request specified {}",
                        b.workspace_id, meta.workspace_id
                    ));
                }
                if let Some(description) = meta.description {
                    b.description = Some(description);
                }
                if let Some(tags) = meta.tags {
                    b.tags = tags;
                }
                if let Some(board_state) = meta.board_state {
                    b.board_state = board_state;
                }
                b.updated_at = Utc::now();
                board = Some((b, ws_id));
                break;
            }
        }
    }

    let (board, ws_id) = board.ok_or_else(|| format!("board not found: {}", board_id))?;

    // Persist updated board
    let json_path = vault::board_json_path(store_root, &ws_id, board_id);
    let json = serde_json::to_string_pretty(&board)
        .map_err(|e| format!("serialize board: {}", e))?;
    std::fs::write(&json_path, json).map_err(|e| format!("write board: {}", e))?;

    Ok(board)
}

/// Read a board from the JSON store.
pub fn get_board(
    store_root: &str,
    workspace_id: &str,
    board_id: &str,
) -> Result<Option<WorkspaceBoard>, String> {
    let json_path = vault::board_json_path(store_root, workspace_id, board_id);
    if !json_path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&json_path).map_err(|e| format!("read board: {}", e))?;
    let board: WorkspaceBoard =
        serde_json::from_str(&content).map_err(|e| format!("parse board: {}", e))?;
    Ok(Some(board))
}

/// List all boards for a workspace.
pub fn list_boards(
    store_root: &str,
    workspace_id: &str,
) -> Result<Vec<WorkspaceBoard>, String> {
    let boards_path = vault::boards_dir(store_root, workspace_id);
    let mut boards = Vec::new();

    if boards_path.exists() {
        for entry in std::fs::read_dir(&boards_path).map_err(|e| format!("list boards: {}", e))?
        {
            let entry = entry.map_err(|e| format!("board entry: {}", e))?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(entry.path())
                    .map_err(|e| format!("read board: {}", e))?;
                let board: WorkspaceBoard = serde_json::from_str(&content)
                    .map_err(|e| format!("parse board: {}", e))?;
                boards.push(board);
            }
        }
    }

    Ok(boards)
}

/// Ingest a board — create an ingest payload receipt.
pub fn ingest_board(
    store_root: &str,
    workspace_id: &str,
    board_id: &str,
    title: &str,
) -> Result<WorkspaceIngestPayload, String> {
    let ingest_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let payload = WorkspaceIngestPayload {
        ingest_id: ingest_id.clone(),
        ingest_scope: "board".to_string(),
        board_id: board_id.to_string(),
        workspace_id: workspace_id.to_string(),
        project_id: None,
        title: title.to_string(),
        summary: String::new(),
        tags: Vec::new(),
        maturity: "draft".to_string(),
        object_ids: Vec::new(),
        connector_ids: Vec::new(),
        object_count: 0,
        connector_count: 0,
        entity_count: 0,
        relation_count: 0,
        graph_status: "ingested".to_string(),
        ingested_at: now,
    };

    // Save to ingests directory
    let ingests_path = vault::ingests_dir(store_root, workspace_id);
    std::fs::create_dir_all(&ingests_path).map_err(|e| format!("create ingests dir: {}", e))?;
    let json_path = ingests_path.join(format!("{}--{}.json", ingest_id, board_id));
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("serialize ingest: {}", e))?;
    std::fs::write(&json_path, json).map_err(|e| format!("write ingest: {}", e))?;

    Ok(payload)
}

/// Ingest a selection of objects/connectors from a board.
pub fn ingest_selection(
    store_root: &str,
    workspace_id: &str,
    board_id: &str,
    title: &str,
    object_ids: Vec<String>,
    connector_ids: Vec<String>,
) -> Result<WorkspaceIngestPayload, String> {
    let ingest_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let payload = WorkspaceIngestPayload {
        ingest_id: ingest_id.clone(),
        ingest_scope: "selection".to_string(),
        board_id: board_id.to_string(),
        workspace_id: workspace_id.to_string(),
        project_id: None,
        title: title.to_string(),
        summary: String::new(),
        tags: Vec::new(),
        maturity: "draft".to_string(),
        object_ids: object_ids.clone(),
        connector_ids: connector_ids.clone(),
        object_count: object_ids.len() as i32,
        connector_count: connector_ids.len() as i32,
        entity_count: 0,
        relation_count: 0,
        graph_status: "ingested".to_string(),
        ingested_at: now,
    };

    // Save to ingests directory
    let ingests_path = vault::ingests_dir(store_root, workspace_id);
    std::fs::create_dir_all(&ingests_path).map_err(|e| format!("create ingests dir: {}", e))?;
    let json_path = ingests_path.join(format!("{}--selection.json", ingest_id));
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("serialize ingest: {}", e))?;
    std::fs::write(&json_path, json).map_err(|e| format!("write ingest: {}", e))?;

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_create_read() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().to_str().unwrap().to_string();

        let board = create_board(&store, "ws-1", "My Board", None, None).unwrap();
        assert_eq!(board.title, "My Board");
        assert_eq!(board.board_type, "canvas");
        assert!(board.tldraw_document.is_object());

        let found = get_board(&store, "ws-1", &board.board_id).unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.board_id, board.board_id);
        assert_eq!(found.title, "My Board");
        assert_eq!(found.object_count, 0);
        assert_eq!(found.connector_count, 0);
    }

    #[test]
    fn test_update_preserves_tldraw() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().to_str().unwrap().to_string();

        let tldraw = serde_json::json!({
            "shapes": ["shape1", "shape2"],
            "bindings": [],
        });

        let board = create_board(&store, "ws-1", "Tldraw Board", None, Some(tldraw.clone())).unwrap();
        let board_id = board.board_id.clone();

        let updated = update_board(
            &store,
            &board_id,
            "Updated Tldraw Board",
            tldraw.clone(),
            vec![],
            vec![],
            BoardUpdateMeta::default(),
        )
        .unwrap();
        assert_eq!(updated.title, "Updated Tldraw Board");
        assert_eq!(updated.tldraw_document, tldraw);
        assert_eq!(updated.object_count, 0);
        assert_eq!(updated.connector_count, 0);
    }

    #[test]
    fn test_update_recalculates_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().to_str().unwrap().to_string();

        let board = create_board(&store, "ws-1", "Count Board", None, None).unwrap();
        let board_id = board.board_id.clone();

        let objects = vec![WorkspaceBoardObject {
            object_id: "obj-1".to_string(),
            board_id: board_id.clone(),
            workspace_id: "ws-1".to_string(),
            project_id: None,
            object_type: "note".to_string(),
            title: Some("Object 1".to_string()),
            summary: None,
            note_id: None,
            asset_id: None,
            artifact_id: None,
            graph_entity_name: None,
            graph_source_id: None,
            tags: Vec::new(),
            ingest_eligible: true,
            locked: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        let connectors = vec![WorkspaceConnector {
            connector_id: "conn-1".to_string(),
            board_id: board_id.clone(),
            workspace_id: "ws-1".to_string(),
            project_id: None,
            from_object_id: "obj-1".to_string(),
            to_object_id: "obj-2".to_string(),
            connector_type: "relation".to_string(),
            relation_intent: "depends_on".to_string(),
            custom_label: None,
            user_authored_summary: None,
            ingest_eligible: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        let updated = update_board(
            &store,
            &board_id,
            "Count Board",
            serde_json::json!({}),
            objects,
            connectors,
            BoardUpdateMeta::default(),
        )
        .unwrap();
        assert_eq!(updated.object_count, 1);
        assert_eq!(updated.connector_count, 1);
    }

    #[test]
    fn test_list_boards() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().to_str().unwrap().to_string();

        create_board(&store, "ws-1", "Board A", None, None).unwrap();
        create_board(&store, "ws-1", "Board B", None, None).unwrap();

        let boards = list_boards(&store, "ws-1").unwrap();
        assert_eq!(boards.len(), 2);
    }

    #[test]
    fn test_ingest_board_creates_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().to_str().unwrap().to_string();

        let payload = ingest_board(&store, "ws-1", "board-123", "My Board").unwrap();
        assert_eq!(payload.ingest_scope, "board");
        assert_eq!(payload.board_id, "board-123");
        assert_eq!(payload.workspace_id, "ws-1");
        assert_eq!(payload.graph_status, "ingested");
    }

    #[test]
    fn test_ingest_selection_creates_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().to_str().unwrap().to_string();

        let payload = ingest_selection(
            &store,
            "ws-1",
            "board-456",
            "My Selection",
            vec!["obj-1".to_string(), "obj-2".to_string()],
            vec!["conn-1".to_string()],
        )
        .unwrap();
        assert_eq!(payload.ingest_scope, "selection");
        assert_eq!(payload.object_count, 2);
        assert_eq!(payload.connector_count, 1);
    }
}
