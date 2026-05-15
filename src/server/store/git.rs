use git2::{Repository, Signature};
use std::path::Path;

/// Ensure a git repo exists at `vault_path`, initializing if needed.
pub fn ensure_repo(vault_path: &Path) -> Result<Repository, git2::Error> {
    if vault_path.join(".git").exists() {
        Repository::open(vault_path)
    } else {
        let repo = Repository::init(vault_path)?;
        // Configure git user
        let mut cfg = repo.config()?;
        cfg.set_str("user.name", "Agentic Memory Workspace")?;
        cfg.set_str("user.email", "workspace@agentmemory.local")?;
        Ok(repo)
    }
}

/// Commit a file change in the vault. Returns the new HEAD SHA.
pub fn commit_file(
    vault_path: &Path,
    relative_path: &str,
    message: &str,
) -> Result<String, git2::Error> {
    let repo = ensure_repo(vault_path)?;
    let mut index = repo.index()?;
    index.add_path(Path::new(relative_path))?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = Signature::now("Agentic Memory Workspace", "workspace@agentmemory.local")?;
    let parent = repo.head()?.peel_to_commit()?;
    let commit_id = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;
    Ok(commit_id.to_string())
}

/// First commit (when repo is empty). Returns the new HEAD SHA.
pub fn first_commit(
    vault_path: &Path,
    relative_path: &str,
    message: &str,
) -> Result<String, git2::Error> {
    let repo = ensure_repo(vault_path)?;
    let mut index = repo.index()?;
    index.add_path(Path::new(relative_path))?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = Signature::now("Agentic Memory Workspace", "workspace@agentmemory.local")?;
    let commit_id = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?;
    Ok(commit_id.to_string())
}

/// Get git log for a specific file path. Returns Vec<(sha, timestamp, subject)>.
pub fn file_history(
    vault_path: &Path,
    relative_path: &str,
) -> Result<Vec<(String, i64, String)>, git2::Error> {
    let repo = Repository::open(vault_path)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    let mut history = Vec::new();
    for oid_result in revwalk {
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        // Check if this commit touches our file
        if commit.parent_count() == 0 {
            history.push((
                oid.to_string(),
                commit.time().seconds(),
                commit.summary().unwrap_or("").to_string(),
            ));
        } else {
            let parent = commit.parent(0)?;
            let tree = commit.tree()?;
            let parent_tree = parent.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
            let mut touches = false;
            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path() {
                        if path == Path::new(relative_path) {
                            touches = true;
                        }
                    }
                    true
                },
                None,
                None,
                None,
            )
            .ok();
            if touches {
                history.push((
                    oid.to_string(),
                    commit.time().seconds(),
                    commit.summary().unwrap_or("").to_string(),
                ));
            }
        }
    }
    Ok(history)
}

/// Get file content at a specific revision.
pub fn show_file_at_rev(
    vault_path: &Path,
    revision: &str,
    relative_path: &str,
) -> Result<String, git2::Error> {
    let repo = Repository::open(vault_path)?;
    let oid = repo.revparse_single(revision)?.id();
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let entry = tree.get_path(Path::new(relative_path))?;
    let blob = repo.find_blob(entry.id())?;
    let content = std::str::from_utf8(blob.content())
        .map_err(|e| git2::Error::from_str(&format!("Invalid UTF-8: {}", e)))?;
    Ok(content.to_string())
}

/// Get current HEAD SHA.
pub fn head_sha(vault_path: &Path) -> Result<String, git2::Error> {
    let repo = Repository::open(vault_path)?;
    let head = repo.head()?.peel_to_commit()?;
    Ok(head.id().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path();

        let _repo = ensure_repo(vault).unwrap();
        assert!(vault.join(".git").exists());

        // Create a file and do first commit
        let file_path = vault.join("test.txt");
        std::fs::write(&file_path, "Hello, world!").unwrap();

        let sha = first_commit(vault, "test.txt", "Initial commit").unwrap();
        assert!(!sha.is_empty());

        let head = head_sha(vault).unwrap();
        assert_eq!(head, sha);
    }

    #[test]
    fn test_file_history() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path();

        ensure_repo(vault).unwrap();

        // First commit
        std::fs::write(vault.join("hist.txt"), "version 1").unwrap();
        first_commit(vault, "hist.txt", "First version").unwrap();

        // Second commit
        std::fs::write(vault.join("hist.txt"), "version 2").unwrap();
        commit_file(vault, "hist.txt", "Second version").unwrap();

        // Third commit
        std::fs::write(vault.join("hist.txt"), "version 3").unwrap();
        commit_file(vault, "hist.txt", "Third version").unwrap();

        let history = file_history(vault, "hist.txt").unwrap();
        // The history includes ALL commits that touch the file
        // The commits are returned from newest to oldest
        assert_eq!(history.len(), 3, "Expected 3 history entries");

        // Verify subjects are present (newest first)
        assert_eq!(history[0].2, "Third version");
        assert_eq!(history[1].2, "Second version");
        assert_eq!(history[2].2, "First version");
    }

    #[test]
    fn test_show_at_rev() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path();

        ensure_repo(vault).unwrap();

        // First commit
        std::fs::write(vault.join("rev.txt"), "original content").unwrap();
        let first_sha = first_commit(vault, "rev.txt", "Original").unwrap();

        // Second commit
        std::fs::write(vault.join("rev.txt"), "modified content").unwrap();
        commit_file(vault, "rev.txt", "Modified").unwrap();

        // Verify current content
        let current = std::fs::read_to_string(vault.join("rev.txt")).unwrap();
        assert_eq!(current, "modified content");

        // Verify we can read the original content at the first revision
        let original = show_file_at_rev(vault, &first_sha, "rev.txt").unwrap();
        assert_eq!(original, "original content");
    }

    #[test]
    fn test_idempotent_ensure_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path();

        // First call initializes
        let _ = ensure_repo(vault).unwrap();
        assert!(vault.join(".git").exists());

        // Second call opens existing
        let _ = ensure_repo(vault).unwrap();
        assert!(vault.join(".git").exists());
    }
}
