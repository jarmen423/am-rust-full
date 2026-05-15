use std::path::{Path, PathBuf};

/// Get the store directory for a workspace.
pub fn store_dir(store_root: &str, workspace_id: &str) -> PathBuf {
    Path::new(store_root).join(workspace_id)
}

/// Get the notes subdirectory.
pub fn notes_dir(store_root: &str, workspace_id: &str) -> PathBuf {
    store_dir(store_root, workspace_id).join("notes")
}

/// Get the boards subdirectory.
pub fn boards_dir(store_root: &str, workspace_id: &str) -> PathBuf {
    store_dir(store_root, workspace_id).join("boards")
}

/// Get the ingests subdirectory.
pub fn ingests_dir(store_root: &str, workspace_id: &str) -> PathBuf {
    store_dir(store_root, workspace_id).join("ingests")
}

/// Get the vault directory for a workspace.
pub fn vault_dir(vault_root: &str, workspace_id: &str) -> PathBuf {
    Path::new(vault_root).join(workspace_id)
}

/// Get the vault notes subdirectory.
pub fn vault_notes_dir(vault_root: &str, workspace_id: &str) -> PathBuf {
    vault_dir(vault_root, workspace_id).join("notes")
}

/// Build a note JSON file path.
pub fn note_json_path(store_root: &str, workspace_id: &str, note_id: &str) -> PathBuf {
    notes_dir(store_root, workspace_id).join(format!("{}.json", note_id))
}

/// Build a board JSON file path.
pub fn board_json_path(store_root: &str, workspace_id: &str, board_id: &str) -> PathBuf {
    boards_dir(store_root, workspace_id).join(format!("{}.json", board_id))
}

/// Build a markdown vault file path from slug + note_id.
pub fn note_vault_path(vault_root: &str, workspace_id: &str, slug: &str, note_id: &str) -> PathBuf {
    vault_notes_dir(vault_root, workspace_id).join(format!("{}--{}.md", slug, note_id))
}

/// Parse a vault filename like "slug--note_id.md" -> (slug, note_id).
pub fn parse_vault_filename(name: &str) -> Option<(String, String)> {
    let name = name.strip_suffix(".md")?;
    let parts: Vec<&str> = name.rsplitn(2, "--").collect();
    if parts.len() == 2 {
        Some((parts[1].to_string(), parts[0].to_string()))
    } else {
        None
    }
}

/// Ensure all directories for a workspace exist.
pub fn ensure_workspace_dirs(
    store_root: &str,
    vault_root: &str,
    workspace_id: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(notes_dir(store_root, workspace_id))?;
    std::fs::create_dir_all(boards_dir(store_root, workspace_id))?;
    std::fs::create_dir_all(ingests_dir(store_root, workspace_id))?;
    std::fs::create_dir_all(vault_notes_dir(vault_root, workspace_id))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_json_path() {
        let path = note_json_path("/tmp/store", "ws-1", "note-abc");
        assert_eq!(path, PathBuf::from("/tmp/store/ws-1/notes/note-abc.json"));
    }

    #[test]
    fn test_board_json_path() {
        let path = board_json_path("/tmp/store", "ws-1", "board-xyz");
        assert_eq!(path, PathBuf::from("/tmp/store/ws-1/boards/board-xyz.json"));
    }

    #[test]
    fn test_note_vault_path() {
        let path = note_vault_path("/tmp/vault", "ws-1", "my-note", "note-123");
        assert_eq!(
            path,
            PathBuf::from("/tmp/vault/ws-1/notes/my-note--note-123.md")
        );
    }

    #[test]
    fn test_parse_vault_filename() {
        let result = parse_vault_filename("my-note--abc123.md").unwrap();
        assert_eq!(result, ("my-note".to_string(), "abc123".to_string()));
    }

    #[test]
    fn test_parse_vault_filename_no_match() {
        let result = parse_vault_filename("just-a-file.md");
        assert!(result.is_none());
    }

    #[test]
    fn test_ensure_workspace_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let store_root = tmp.path().join("store").to_str().unwrap().to_string();
        let vault_root = tmp.path().join("vault").to_str().unwrap().to_string();
        ensure_workspace_dirs(&store_root, &vault_root, "ws-test").unwrap();

        assert!(std::fs::metadata(format!("{}/ws-test/notes", store_root)).is_ok());
        assert!(std::fs::metadata(format!("{}/ws-test/boards", store_root)).is_ok());
        assert!(std::fs::metadata(format!("{}/ws-test/ingests", store_root)).is_ok());
        assert!(std::fs::metadata(format!("{}/ws-test/notes", vault_root)).is_ok());
    }
}
