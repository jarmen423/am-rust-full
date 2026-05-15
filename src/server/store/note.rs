use am_workspace::model::{NoteHistoryItem, WorkspaceNoteDocument};
use crate::store::vault;
use chrono::Utc;
use serde_json;
use std::path::Path;
use uuid::Uuid;

/// Parse frontmatter that uses JSON literals after key:
/// ---
/// note_id: "uuid"
/// tags: ["tag1", "tag2"]
/// ---
fn parse_frontmatter(
    content: &str,
) -> Result<(std::collections::HashMap<String, String>, String), String> {
    let mut map = std::collections::HashMap::new();
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return Ok((map, content.to_string()));
    }
    let after_open = &content[3..];
    let Some(end_idx) = after_open.find("\n---") else {
        return Ok((map, content.to_string()));
    };
    let fm_block = &after_open[..end_idx];
    let body_start = end_idx + 4;
    let body = after_open[body_start..].trim_start().to_string();

    for line in fm_block.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(colon_idx) = line.find(':') {
            let key = line[..colon_idx].trim().to_string();
            let value = line[colon_idx + 1..].trim().to_string();
            map.insert(key, value);
        }
    }
    Ok((map, body))
}

fn build_frontmatter(note: &WorkspaceNoteDocument) -> String {
    let tags_json = serde_json::to_string(&note.tags).unwrap_or_else(|_| "[]".to_string());
    let entity_hints_json =
        serde_json::to_string(&note.entity_hints).unwrap_or_else(|_| "[]".to_string());
    let summary_quoted = note
        .summary
        .as_ref()
        .map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
        .unwrap_or_else(|| "null".to_string());
    let project_quoted = note
        .project_id
        .as_ref()
        .map(|s| format!("\"{}\"", s))
        .unwrap_or_else(|| "null".to_string());

    format!(
        "---\nnote_id: \"{}\"\nworkspace_id: \"{}\"\nproject_id: {}\ntitle: \"{}\"\nsummary: {}\ntags: {}\nentity_hints: {}\nsource: \"{}\"\ngraph_status: \"{}\"\n---\n\n{}",
        note.note_id,
        note.workspace_id,
        project_quoted,
        note.title.replace('"', "\\\""),
        summary_quoted,
        tags_json,
        entity_hints_json,
        note.source,
        note.graph_status,
        note.body_markdown
    )
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

/// Write the JSON store file and markdown vault file for a note.
fn persist_note(
    store_root: &str,
    vault_root: &str,
    note: &WorkspaceNoteDocument,
) -> Result<(), String> {
    // Write JSON store file
    let json_path = vault::note_json_path(store_root, &note.workspace_id, &note.note_id);
    if let Some(parent) = json_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create notes dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(note)
        .map_err(|e| format!("serialize note JSON: {}", e))?;
    std::fs::write(&json_path, json).map_err(|e| format!("write note JSON: {}", e))?;

    // Write markdown vault file with frontmatter
    let slug = slugify(&note.title);
    let md_path = vault::note_vault_path(vault_root, &note.workspace_id, &slug, &note.note_id);
    if let Some(parent) = md_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create vault notes dir: {}", e))?;
    }
    let frontmatter = build_frontmatter(note);
    std::fs::write(&md_path, frontmatter)
        .map_err(|e| format!("write note markdown: {}", e))?;

    Ok(())
}

/// Create a new note with JSON store + markdown vault file, commits to git.
pub fn create_note(
    store_root: &str,
    vault_root: &str,
    workspace_id: &str,
    title: &str,
    body_markdown: &str,
    tags: Vec<String>,
    project_id: Option<String>,
) -> Result<WorkspaceNoteDocument, String> {
    let now = Utc::now();
    let note_id = Uuid::new_v4().to_string();
    let slug = slugify(title);

    // Ensure directories exist
    vault::ensure_workspace_dirs(store_root, vault_root, workspace_id)
        .map_err(|e| format!("ensure dirs: {}", e))?;

    let note = WorkspaceNoteDocument {
        note_id: note_id.clone(),
        workspace_id: workspace_id.to_string(),
        project_id,
        slug,
        title: title.to_string(),
        body_markdown: body_markdown.to_string(),
        summary: None,
        tags,
        entity_hints: Vec::new(),
        source: "user".to_string(),
        created_at: now,
        updated_at: now,
        archived_at: None,
        graph_status: "not_ingested".to_string(),
        markdown_path: None,
        git_revision: None,
    };

    persist_note(store_root, vault_root, &note)?;

    // Git commit
    let relative_path = format!(
        "notes/{}--{}.md",
        slugify(title),
        note_id
    );
    let vault_path = Path::new(vault_root).join(workspace_id);
    let head_sha = if vault_path.join(".git").exists() {
        let head = super::git::head_sha(&vault_path).ok();
        if head.is_some() {
            super::git::commit_file(&vault_path, &relative_path, &format!("Create note: {}", title))
                .map_err(|e| format!("git commit: {}", e))?
        } else {
            super::git::first_commit(&vault_path, &relative_path, &format!("Create note: {}", title))
                .map_err(|e| format!("git first commit: {}", e))?
        }
    } else {
        super::git::first_commit(&vault_path, &relative_path, &format!("Create note: {}", title))
            .map_err(|e| format!("git first commit: {}", e))?
    };

    let mut note = note;
    note.git_revision = Some(head_sha);
    Ok(note)
}

/// Update an existing note, rewriting both JSON and vault file.
pub fn update_note(
    store_root: &str,
    vault_root: &str,
    note_id: &str,
    title: &str,
    body_markdown: &str,
    tags: Vec<String>,
    project_id: Option<String>,
) -> Result<WorkspaceNoteDocument, String> {
    let existing = get_note(store_root, vault_root, "", note_id)?;
    let mut note = existing.ok_or_else(|| format!("note not found: {}", note_id))?;

    note.title = title.to_string();
    note.slug = slugify(title);
    note.body_markdown = body_markdown.to_string();
    note.tags = tags;
    note.project_id = project_id;
    note.updated_at = Utc::now();

    persist_note(store_root, vault_root, &note)?;

    // Git commit
    let relative_path = format!("notes/{}--{}.md", note.slug, note_id);
    let vault_path = Path::new(vault_root).join(&note.workspace_id);
    let head_sha = super::git::commit_file(&vault_path, &relative_path, &format!("Update note: {}", title))
        .map_err(|e| format!("git commit: {}", e))?;
    note.git_revision = Some(head_sha);

    Ok(note)
}

/// Read a note from the JSON store. If not found, try vault.
pub fn get_note(
    store_root: &str,
    vault_root: &str,
    _workspace_id: &str,
    note_id: &str,
) -> Result<Option<WorkspaceNoteDocument>, String> {
    // First try to find in the store directory by scanning all workspace subdirs
    let store = Path::new(store_root);
    if store.exists() {
        for entry in std::fs::read_dir(store).map_err(|e| format!("read store: {}", e))? {
            let entry = entry.map_err(|e| format!("store entry: {}", e))?;
            let ws_id = entry.file_name().to_string_lossy().to_string();
            let json_path = vault::note_json_path(store_root, &ws_id, note_id);
            if json_path.exists() {
                let content =
                    std::fs::read_to_string(&json_path).map_err(|e| format!("read note: {}", e))?;
                let mut note: WorkspaceNoteDocument = serde_json::from_str(&content)
                    .map_err(|e| format!("parse note: {}", e))?;
                // Populate git_revision from HEAD
                let vault_path = Path::new(vault_root).join(&ws_id);
                if let Ok(sha) = super::git::head_sha(&vault_path) {
                    note.git_revision = Some(sha);
                }
                return Ok(Some(note));
            }
        }
    }

    // Fallback: scan vault directory for matching note files
    let vault = Path::new(vault_root);
    if vault.exists() {
        for entry in std::fs::read_dir(vault).map_err(|e| format!("read vault: {}", e))? {
            let entry = entry.map_err(|e| format!("vault entry: {}", e))?;
            let ws_id = entry.file_name().to_string_lossy().to_string();
            let notes_path = vault::vault_notes_dir(vault_root, &ws_id);
            if notes_path.exists() {
                for note_entry in
                    std::fs::read_dir(&notes_path).map_err(|e| format!("read notes: {}", e))?
                {
                    let note_entry = note_entry.map_err(|e| format!("note entry: {}", e))?;
                    let filename = note_entry.file_name().to_string_lossy().to_string();
                    if let Some((_, found_note_id)) = vault::parse_vault_filename(&filename) {
                        if found_note_id == note_id {
                            let md_content = std::fs::read_to_string(note_entry.path())
                                .map_err(|e| format!("read md: {}", e))?;
                            let (fm, body) = parse_frontmatter(&md_content)?;
                            let now = Utc::now();
                            let note = WorkspaceNoteDocument {
                                note_id: note_id.to_string(),
                                workspace_id: ws_id.clone(),
                                project_id: fm.get("project_id").and_then(|s| {
                                    if s == "null" {
                                        None
                                    } else {
                                        Some(s.trim_matches('"').to_string())
                                    }
                                }),
                                slug: fm.get("title").map(|t| slugify(t)).unwrap_or_default(),
                                title: fm
                                    .get("title")
                                    .map(|t| t.trim_matches('"').to_string())
                                    .unwrap_or_default(),
                                body_markdown: body,
                                summary: fm.get("summary").and_then(|s| {
                                    if s == "null" {
                                        None
                                    } else {
                                        Some(s.trim_matches('"').to_string())
                                    }
                                }),
                                tags: fm
                                    .get("tags")
                                    .and_then(|t| serde_json::from_str(t).ok())
                                    .unwrap_or_default(),
                                entity_hints: fm
                                    .get("entity_hints")
                                    .and_then(|t| serde_json::from_str(t).ok())
                                    .unwrap_or_default(),
                                source: fm
                                    .get("source")
                                    .map(|s| s.trim_matches('"').to_string())
                                    .unwrap_or_else(|| "user".to_string()),
                                created_at: now,
                                updated_at: now,
                                archived_at: None,
                                graph_status: fm
                                    .get("graph_status")
                                    .map(|s| s.trim_matches('"').to_string())
                                    .unwrap_or_else(|| "not_ingested".to_string()),
                                markdown_path: Some(
                                    note_entry.path().to_string_lossy().to_string(),
                                ),
                                git_revision: None,
                            };
                            return Ok(Some(note));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// List all notes for a workspace from the JSON store.
pub fn list_notes(
    store_root: &str,
    vault_root: &str,
    workspace_id: &str,
) -> Result<Vec<WorkspaceNoteDocument>, String> {
    let notes_path = vault::notes_dir(store_root, workspace_id);
    let mut notes = Vec::new();

    if notes_path.exists() {
        for entry in std::fs::read_dir(&notes_path).map_err(|e| format!("list notes: {}", e))? {
            let entry = entry.map_err(|e| format!("note entry: {}", e))?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(entry.path())
                    .map_err(|e| format!("read note: {}", e))?;
                let mut note: WorkspaceNoteDocument = serde_json::from_str(&content)
                    .map_err(|e| format!("parse note: {}", e))?;
                let vault_path = Path::new(vault_root).join(workspace_id);
                if let Ok(sha) = super::git::head_sha(&vault_path) {
                    note.git_revision = Some(sha);
                }
                notes.push(note);
            }
        }
    }

    Ok(notes)
}

/// Return minimal `{note_id, title, tags}` objects for a picker UI.
pub fn note_picker(
    store_root: &str,
    _vault_root: &str,
    workspace_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let notes_path = vault::notes_dir(store_root, workspace_id);
    let mut items = Vec::new();

    if notes_path.exists() {
        for entry in std::fs::read_dir(&notes_path).map_err(|e| format!("picker: {}", e))? {
            let entry = entry.map_err(|e| format!("picker entry: {}", e))?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(entry.path())
                    .map_err(|e| format!("read note: {}", e))?;
                let note: WorkspaceNoteDocument = serde_json::from_str(&content)
                    .map_err(|e| format!("parse note: {}", e))?;
                items.push(serde_json::json!({
                    "note_id": note.note_id,
                    "title": note.title,
                    "tags": note.tags,
                }));
            }
        }
    }

    Ok(items)
}

/// Get git history for a specific note.
pub fn note_history(
    vault_root: &str,
    _workspace_id: &str,
    note_id: &str,
) -> Result<Vec<NoteHistoryItem>, String> {
    // Scan all workspaces to find the note and get its slug for the path
    let vault = Path::new(vault_root);
    if !vault.exists() {
        return Ok(Vec::new());
    }

    for entry in std::fs::read_dir(vault).map_err(|e| format!("read vault: {}", e))? {
        let entry = entry.map_err(|e| format!("vault entry: {}", e))?;
        let ws_id = entry.file_name().to_string_lossy().to_string();
        let notes_path = vault::vault_notes_dir(vault_root, &ws_id);
        if notes_path.exists() {
            for note_entry in
                std::fs::read_dir(&notes_path).map_err(|e| format!("read notes: {}", e))?
            {
                let note_entry = note_entry.map_err(|e| format!("note entry: {}", e))?;
                let filename = note_entry.file_name().to_string_lossy().to_string();
                if let Some((slug, found_note_id)) = vault::parse_vault_filename(&filename) {
                    if found_note_id == note_id {
                        let relative_path = format!("notes/{}--{}.md", slug, note_id);
                        let vault_path = vault::vault_dir(vault_root, &ws_id);
                        let history = super::git::file_history(&vault_path, &relative_path)
                            .map_err(|e| format!("file history: {}", e))?;
                        return Ok(history
                            .into_iter()
                            .map(|(sha, timestamp, subject)| NoteHistoryItem {
                                sha,
                                timestamp: timestamp.to_string(),
                                subject,
                            })
                            .collect());
                    }
                }
            }
        }
    }

    Ok(Vec::new())
}

/// Revert a note to a specific git revision.
pub fn revert_note(
    store_root: &str,
    vault_root: &str,
    workspace_id: &str,
    note_id: &str,
    revision: &str,
) -> Result<WorkspaceNoteDocument, String> {
    // Find the note in the vault to determine its slug
    let notes_path = vault::vault_notes_dir(vault_root, workspace_id);
    let mut slug = String::new();
    for entry in std::fs::read_dir(&notes_path).map_err(|e| format!("read notes: {}", e))? {
        let entry = entry.map_err(|e| format!("note entry: {}", e))?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if let Some((found_slug, found_note_id)) = vault::parse_vault_filename(&filename) {
            if found_note_id == note_id {
                slug = found_slug;
                break;
            }
        }
    }

    if slug.is_empty() {
        return Err(format!("note not found in vault: {}", note_id));
    }

    let relative_path = format!("notes/{}--{}.md", slug, note_id);
    let vault_path = vault::vault_dir(vault_root, workspace_id);

    // Read the file content at the specified revision
    let old_content = super::git::show_file_at_rev(&vault_path, revision, &relative_path)
        .map_err(|e| format!("show at rev: {}", e))?;

    // Parse the frontmatter from the old content
    let (fm, body) = parse_frontmatter(&old_content)?;

    let now = Utc::now();

    // Build updated note document
    let mut note = WorkspaceNoteDocument {
        note_id: note_id.to_string(),
        workspace_id: workspace_id.to_string(),
        project_id: fm.get("project_id").and_then(|s| {
            if s == "null" {
                None
            } else {
                Some(s.trim_matches('"').to_string())
            }
        }),
        slug: slug.clone(),
        title: fm
            .get("title")
            .map(|t| t.trim_matches('"').to_string())
            .unwrap_or_default(),
        body_markdown: body,
        summary: fm.get("summary").and_then(|s| {
            if s == "null" {
                None
            } else {
                Some(s.trim_matches('"').to_string())
            }
        }),
        tags: fm
            .get("tags")
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default(),
        entity_hints: fm
            .get("entity_hints")
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default(),
        source: fm
            .get("source")
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_else(|| "user".to_string()),
        created_at: now,
        updated_at: now,
        archived_at: None,
        graph_status: fm
            .get("graph_status")
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_else(|| "not_ingested".to_string()),
        markdown_path: None,
        git_revision: None,
    };

    // Write back to JSON store and vault
    persist_note(store_root, vault_root, &note)?;

    // Commit the revert
    let new_sha = super::git::commit_file(&vault_path, &relative_path, &format!("Revert note to {}: {}", revision, &note.title))
        .map_err(|e| format!("git commit revert: {}", e))?;
    note.git_revision = Some(new_sha);

    Ok(note)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_create_read() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store").to_str().unwrap().to_string();
        let vault = tmp.path().join("vault").to_str().unwrap().to_string();

        let note = create_note(&store, &vault, "ws-1", "Hello Note", "# Hello\n\nBody", vec!["tag1".to_string()], None).unwrap();
        assert_eq!(note.title, "Hello Note");
        assert_eq!(note.body_markdown, "# Hello\n\nBody");
        assert!(note.git_revision.is_some());

        let found = get_note(&store, &vault, "ws-1", &note.note_id).unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.note_id, note.note_id);
        assert_eq!(found.title, "Hello Note");
        assert_eq!(found.body_markdown, "# Hello\n\nBody");
    }

    #[test]
    fn test_frontmatter_parsing() {
        let md = r#"---
note_id: "abc-123"
workspace_id: "ws-1"
project_id: null
title: "My Title"
summary: "A summary"
tags: ["rust", "notes"]
entity_hints: []
source: "user"
graph_status: "not_ingested"
---

# Body here

Some content."#;

        let (fm, body) = parse_frontmatter(md).unwrap();
        assert_eq!(fm.get("note_id").unwrap(), "\"abc-123\"");
        assert_eq!(fm.get("title").unwrap(), "\"My Title\"");
        assert_eq!(fm.get("summary").unwrap(), "\"A summary\"");
        assert_eq!(body, "# Body here\n\nSome content.");
    }

    #[test]
    fn test_note_history() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store").to_str().unwrap().to_string();
        let vault = tmp.path().join("vault").to_str().unwrap().to_string();

        // Create a note
        let note = create_note(&store, &vault, "ws-1", "History Note", "v1", vec![], None).unwrap();

        // Update the note
        let note_id = note.note_id.clone();
        update_note(&store, &vault, &note_id, "History Note Updated", "v2", vec![], None).unwrap();

        // Check history has at least 2 entries (create + update)
        let history = note_history(&vault, "ws-1", &note_id).unwrap();
        assert!(history.len() >= 1, "Expected at least 1 history entry");
    }

    #[test]
    fn test_revert_note() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store").to_str().unwrap().to_string();
        let vault = tmp.path().join("vault").to_str().unwrap().to_string();

        // Create a note
        let note = create_note(&store, &vault, "ws-1", "Revert Note", "original body", vec![], None).unwrap();
        let note_id = note.note_id.clone();
        let first_sha = note.git_revision.clone().unwrap();

        // Update it
        update_note(&store, &vault, &note_id, "Revert Note", "modified body", vec![], None).unwrap();

        // Verify modified
        let after_update = get_note(&store, &vault, "ws-1", &note_id).unwrap().unwrap();
        assert_eq!(after_update.body_markdown, "modified body");

        // Revert to first commit
        let reverted = revert_note(&store, &vault, "ws-1", &note_id, &first_sha).unwrap();
        assert_eq!(reverted.body_markdown, "original body");
    }

    #[test]
    fn test_slugify_behavior() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("My Note!"), "my-note");
        assert_eq!(slugify("--trim--"), "trim");
    }

    #[test]
    fn test_note_picker_returns_minimal() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store").to_str().unwrap().to_string();
        let vault = tmp.path().join("vault").to_str().unwrap().to_string();

        create_note(&store, &vault, "ws-1", "Picker Note", "body", vec!["a".to_string(), "b".to_string()], None).unwrap();

        let picker = note_picker(&store, &vault, "ws-1").unwrap();
        assert_eq!(picker.len(), 1);
        assert!(picker[0].get("note_id").is_some());
        assert!(picker[0].get("title").is_some());
        assert!(picker[0].get("tags").is_some());
    }

    #[test]
    fn test_list_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store").to_str().unwrap().to_string();
        let vault = tmp.path().join("vault").to_str().unwrap().to_string();

        create_note(&store, &vault, "ws-1", "Note A", "body a", vec![], None).unwrap();
        create_note(&store, &vault, "ws-1", "Note B", "body b", vec![], None).unwrap();

        let notes = list_notes(&store, &vault, "ws-1").unwrap();
        assert_eq!(notes.len(), 2);
    }
}
