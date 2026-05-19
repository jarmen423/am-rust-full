//! LadybugDB graph query functions for Phase 4.
//!
//! Handlers pass `&LadybugDb` when a `.lbug` file is available. Without native
//! Ladybug (`--no-default-features`, no `ladybug` feature), `LadybugDb` wraps
//! the internal [`crate::lbug_shim`] whose queries always return empty rows so
//! routes never panic on missing DLLs.
//!
//! With the default `ladybug` feature, this module links the official [`lbug`]
//! crate and executes real Cypher against an on-disk database opened in
//! **read-only** mode for safer local inspection of hosted snapshots.

use std::path::PathBuf;

use am_workspace::model::{WorkspaceGraphEdge, WorkspaceGraphNode};
use serde_json::json;

// ── Ladybug runtime handle (native vs shim) ──────────────────────────

/// Handle to an on-disk Ladybug database used by graph routes.
///
/// Cloning is cheap: native builds wrap `Arc<lbug::Database>`; shim builds
/// clone the lightweight stub connection.
#[cfg(feature = "ladybug")]
#[derive(Clone)]
pub struct LadybugDb {
    /// Resolved filesystem path (for logs / diagnostics).
    pub path: PathBuf,
    db: std::sync::Arc<lbug::Database>,
}

#[cfg(feature = "ladybug")]
impl std::fmt::Debug for LadybugDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LadybugDb")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

#[cfg(not(feature = "ladybug"))]
#[derive(Clone, Debug)]
pub struct LadybugDb {
    pub path: PathBuf,
    pub(crate) conn: crate::lbug_shim::Connection,
}

#[cfg(feature = "ladybug")]
fn value_as_required_string(v: &lbug::Value) -> Result<String, String> {
    match v {
        lbug::Value::Null(_) => Err("unexpected NULL in required column".to_string()),
        lbug::Value::String(s) => Ok(s.clone()),
        lbug::Value::Bool(b) => Ok(b.to_string()),
        lbug::Value::Int64(i) => Ok(i.to_string()),
        lbug::Value::Int32(i) => Ok(i.to_string()),
        lbug::Value::Int16(i) => Ok(i.to_string()),
        lbug::Value::Int8(i) => Ok(i.to_string()),
        lbug::Value::UInt64(u) => Ok(u.to_string()),
        lbug::Value::UInt32(u) => Ok(u.to_string()),
        lbug::Value::UInt16(u) => Ok(u.to_string()),
        lbug::Value::UInt8(u) => Ok(u.to_string()),
        lbug::Value::Float(x) => Ok(x.to_string()),
        lbug::Value::Double(x) => Ok(x.to_string()),
        other => Ok(other.to_string()),
    }
}

#[cfg(feature = "ladybug")]
fn value_as_optional_string(v: &lbug::Value) -> Option<String> {
    match v {
        lbug::Value::Null(_) => None,
        lbug::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(feature = "ladybug")]
fn value_as_required_i64(v: &lbug::Value) -> Result<i64, String> {
    match v {
        lbug::Value::Int64(i) => Ok(*i),
        lbug::Value::Int32(i) => Ok(*i as i64),
        lbug::Value::Int16(i) => Ok(*i as i64),
        lbug::Value::Int8(i) => Ok(*i as i64),
        lbug::Value::UInt64(u) => Ok(*u as i64),
        lbug::Value::UInt32(u) => Ok(*u as i64),
        lbug::Value::UInt16(u) => Ok(*u as i64),
        lbug::Value::UInt8(u) => Ok(*u as i64),
        other => Err(format!("expected integral count column, got {:?}", other)),
    }
}

/// Run a query whose rows are three string columns (typical rel sample rows).
#[cfg(feature = "ladybug")]
fn query_rows_3(db: &LadybugDb, sql: &str) -> Result<Vec<(String, String, String)>, String> {
    let conn = lbug::Connection::new(&db.db).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in conn.query(sql).map_err(|e| e.to_string())? {
        if row.len() != 3 {
            return Err(format!("expected 3 columns, got {}", row.len()));
        }
        out.push((
            value_as_required_string(&row[0])?,
            value_as_required_string(&row[1])?,
            value_as_required_string(&row[2])?,
        ));
    }
    Ok(out)
}

#[cfg(not(feature = "ladybug"))]
fn query_rows_3(db: &LadybugDb, sql: &str) -> Result<Vec<(String, String, String)>, String> {
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    stmt.query_map(&[], |row| {
        Ok((row.get::<String>(0)?, row.get::<String>(1)?, row.get::<String>(2)?))
    })
    .map_err(|e| e.to_string())
}

#[cfg(feature = "ladybug")]
fn query_entity_edges(db: &LadybugDb, sql: &str) -> Result<Vec<WorkspaceGraphEdge>, String> {
    let conn = lbug::Connection::new(&db.db).map_err(|e| e.to_string())?;
    let mut edges = Vec::new();
    for row in conn.query(sql).map_err(|e| e.to_string())? {
        if row.len() != 3 {
            return Err(format!("expected 3 columns, got {}", row.len()));
        }
        let source = value_as_required_string(&row[0])?;
        let target = value_as_required_string(&row[1])?;
        let rel_type = value_as_required_string(&row[2])?;
        let from_id = format!("ladybug-entity:{}", source);
        let to_id = format!("ladybug-entity:{}", target);
        edges.push(make_edge(&from_id, &to_id, &rel_type));
    }
    Ok(edges)
}

#[cfg(not(feature = "ladybug"))]
fn query_entity_edges(db: &LadybugDb, sql: &str) -> Result<Vec<WorkspaceGraphEdge>, String> {
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    stmt.query_map(&[], |row| {
        let source: String = row.get(0)?;
        let target: String = row.get(1)?;
        let rel_type: String = row.get(2)?;
        let from_id = format!("ladybug-entity:{}", source);
        let to_id = format!("ladybug-entity:{}", target);
        Ok(make_edge(&from_id, &to_id, &rel_type))
    })
    .map_err(|e| e.to_string())
}

#[cfg(feature = "ladybug")]
fn query_node_detail_rows(db: &LadybugDb, sql: &str) -> Result<Vec<WorkspaceGraphNode>, String> {
    let conn = lbug::Connection::new(&db.db).map_err(|e| e.to_string())?;
    let mut nodes = Vec::new();
    for row in conn.query(sql).map_err(|e| e.to_string())? {
        if row.len() != 8 {
            return Err(format!("expected 8 columns, got {}", row.len()));
        }
        let node_id = value_as_required_string(&row[0])?;
        let primary_label = value_as_required_string(&row[1])?;
        let name = value_as_required_string(&row[2])?;
        let path = value_as_optional_string(&row[3]);
        let qualified_name = value_as_optional_string(&row[4]);
        let repo_id = value_as_optional_string(&row[5]);
        let project_id = value_as_optional_string(&row[6]);
        let text = value_as_optional_string(&row[7]);
        nodes.push(make_ladybug_node(
            &node_id,
            &primary_label,
            &name,
            path.as_deref(),
            qualified_name.as_deref(),
            repo_id.as_deref(),
            project_id.as_deref(),
            text.as_deref(),
        ));
    }
    Ok(nodes)
}

#[cfg(not(feature = "ladybug"))]
fn query_node_detail_rows(db: &LadybugDb, sql: &str) -> Result<Vec<WorkspaceGraphNode>, String> {
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    stmt.query_map(&[], |row| {
        let node_id: String = row.get(0)?;
        let primary_label: String = row.get(1)?;
        let name: String = row.get(2)?;
        let path: Option<String> = row.get(3).ok();
        let qualified_name: Option<String> = row.get(4).ok();
        let repo_id: Option<String> = row.get(5).ok();
        let project_id: Option<String> = row.get(6).ok();
        let text: Option<String> = row.get(7).ok();
        Ok(make_ladybug_node(
            &node_id,
            &primary_label,
            &name,
            path.as_deref(),
            qualified_name.as_deref(),
            repo_id.as_deref(),
            project_id.as_deref(),
            text.as_deref(),
        ))
    })
    .map_err(|e| e.to_string())
}

#[cfg(feature = "ladybug")]
fn query_single_string_column(db: &LadybugDb, sql: &str) -> Result<Vec<String>, String> {
    let conn = lbug::Connection::new(&db.db).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in conn.query(sql).map_err(|e| e.to_string())? {
        if row.len() != 1 {
            return Err(format!("expected 1 column, got {}", row.len()));
        }
        out.push(value_as_required_string(&row[0])?);
    }
    Ok(out)
}

#[cfg(not(feature = "ladybug"))]
fn query_single_string_column(db: &LadybugDb, sql: &str) -> Result<Vec<String>, String> {
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    stmt.query_map(&[], |row| row.get::<String>(0))
        .map_err(|e| e.to_string())
}

#[cfg(feature = "ladybug")]
fn query_repo_project_counts(db: &LadybugDb, sql: &str) -> Result<Vec<(String, i64)>, String> {
    let conn = lbug::Connection::new(&db.db).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in conn.query(sql).map_err(|e| e.to_string())? {
        if row.len() != 2 {
            return Err(format!("expected 2 columns, got {}", row.len()));
        }
        let id = value_as_required_string(&row[0])?;
        let count = value_as_required_i64(&row[1])?;
        out.push((id, count));
    }
    Ok(out)
}

#[cfg(not(feature = "ladybug"))]
fn query_repo_project_counts(db: &LadybugDb, sql: &str) -> Result<Vec<(String, i64)>, String> {
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    stmt.query_map(&[], |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?)))
        .map_err(|e| e.to_string())
}

// ── Constants ────────────────────────────────────────────────────────

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "from", "that", "this", "note", "board",
];

// ── Cypher escaping ──────────────────────────────────────────────────

/// Escape a string value for safe interpolation into a Cypher query.
///
/// Cypher uses double-quotes for string literals; backslash is the escape
/// character.  We escape both `"` and `\`.
pub fn escape_cypher(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── Keyword extraction ───────────────────────────────────────────────

/// Extract up to `max` keywords from a title.
///
/// Keywords are words of at least 3 characters, lower-cased, with common
/// stop words removed.  Used for Ladybug entity enrichment in board/note
/// graph handlers.
pub fn extract_keywords(title: &str, max: usize) -> Vec<String> {
    let mut keywords = Vec::new();
    for word in title.split_whitespace() {
        let w = word.to_lowercase();
        let w = w.trim_matches(|c: char| !c.is_alphanumeric());
        if w.len() >= 3 && !STOP_WORDS.contains(&w) {
            if !keywords.contains(&w.to_string()) {
                keywords.push(w.to_string());
            }
        }
        if keywords.len() >= max {
            break;
        }
    }
    keywords
}

// ── Database path discovery ──────────────────────────────────────────

/// Discover the `.lbug` database file path.
///
/// Search order:
/// 1. `LADYBUG_DB_PATH` environment variable
/// 2. `~/.agentic-memory/*.lbug`
/// 3. `<store_root>/*.lbug`
///
/// Returns `None` if no `.lbug` file is found.
pub fn find_lbug_db_path(store_root: &str) -> Option<String> {
    // 1. Explicit env override
    if let Ok(path) = std::env::var("LADYBUG_DB_PATH") {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    // 2. ~/.agentic-memory/*.lbug (HOME or USERPROFILE on Windows)
    if let Some(home) = std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
    {
        let am_dir = std::path::Path::new(&home).join(".agentic-memory");
        if let Some(path) = find_first_lbug(&am_dir) {
            return Some(path);
        }
    }

    // 3. <store_root>/*.lbug
    if let Some(path) = find_first_lbug(std::path::Path::new(store_root)) {
        return Some(path);
    }

    None
}

fn find_first_lbug(dir: &std::path::Path) -> Option<String> {
    if !dir.exists() || !dir.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("lbug") {
            return Some(path.to_string_lossy().to_string());
        }
    }
    // Also search one level deep
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_first_lbug(&path) {
                return Some(found);
            }
        }
    }
    None
}

/// Open a Ladybug database handle if a `.lbug` file can be found.
///
/// Native builds open read-only to reduce accidental mutation while browsing.
#[cfg(feature = "ladybug")]
pub fn open_ladybug_db(store_root: &str) -> Option<LadybugDb> {
    let path_str = find_lbug_db_path(store_root)?;
    let path = PathBuf::from(&path_str);
    let cfg = lbug::SystemConfig::default().read_only(true);
    match lbug::Database::new(&path_str, cfg) {
        Ok(db) => {
            tracing::info!(db_path = %path_str, "LadybugDB opened (native lbug, read-only)");
            Some(LadybugDb {
                path,
                db: std::sync::Arc::new(db),
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, db_path = %path_str, "Failed to open LadybugDB");
            None
        }
    }
}

#[cfg(not(feature = "ladybug"))]
pub fn open_ladybug_db(store_root: &str) -> Option<LadybugDb> {
    let path_str = find_lbug_db_path(store_root)?;
    let path = PathBuf::from(&path_str);
    match crate::lbug_shim::Connection::open(&path_str) {
        Ok(conn) => {
            tracing::info!(db_path = %path_str, "LadybugDB shim handle opened (queries return empty)");
            Some(LadybugDb { path, conn })
        }
        Err(e) => {
            tracing::warn!(error = %e, db_path = %path_str, "Failed to open LadybugDB shim");
            None
        }
    }
}

// ── Node / edge builders ─────────────────────────────────────────────

/// Build a `WorkspaceGraphNode` from a LadybugDB result row.
fn make_ladybug_node(
    node_id: &str,
    primary_label: &str,
    name: &str,
    path: Option<&str>,
    qualified_name: Option<&str>,
    repo_id: Option<&str>,
    project_id: Option<&str>,
    text: Option<&str>,
) -> WorkspaceGraphNode {
    let title = if name.is_empty() {
        node_id.to_string()
    } else {
        name.to_string()
    };
    let subtitle = qualified_name.map(|s| s.to_string());

    let mut metadata = json!({
        "source": "ladybug",
        "primary_label": primary_label,
    });
    if let Some(p) = path {
        metadata["path"] = json!(p);
    }
    if let Some(q) = qualified_name {
        metadata["qualified_name"] = json!(q);
    }
    if let Some(r) = repo_id {
        metadata["repo_id"] = json!(r);
    }
    if let Some(p) = project_id {
        metadata["project_id"] = json!(p);
    }
    if let Some(t) = text {
        metadata["text"] = json!(t);
    }

    WorkspaceGraphNode {
        node_id: format!("ladybug:{}", node_id),
        node_type: primary_label.to_string(),
        title,
        subtitle,
        metadata,
    }
}

/// Build a `WorkspaceGraphEdge` from endpoint IDs and a relation type.
fn make_edge(from_node_id: &str, to_node_id: &str, rel_type: &str) -> WorkspaceGraphEdge {
    let edge_id = format!("{}--{}--{}", from_node_id, rel_type, to_node_id);
    WorkspaceGraphEdge {
        edge_id,
        from_node_id: from_node_id.to_string(),
        to_node_id: to_node_id.to_string(),
        relation_type: rel_type.to_string(),
        label: None,
        metadata: json!({"source": "ladybug"}),
    }
}

fn make_entity_node(name: &str) -> WorkspaceGraphNode {
    WorkspaceGraphNode {
        node_id: format!("ladybug-entity:{}", name),
        node_type: "MemoryEntity".to_string(),
        title: name.to_string(),
        subtitle: None,
        metadata: json!({"source": "ladybug", "primary_label": "MemoryEntity"}),
    }
}

// ── Explore graph ────────────────────────────────────────────────────

/// Random sample of the graph, optionally scoped by repo/project.
///
/// 1. Sample relationships (scoped or unscoped).
/// 2. Fetch node details for all endpoints.
/// 3. Density boost — add all relationships among collected endpoints.
pub fn explore_graph(
    db: &LadybugDb,
    limit: i64,
    repo_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<(Vec<WorkspaceGraphNode>, Vec<WorkspaceGraphEdge>), String> {
    // Step 1: sample relationships
    let sample_sql = if let (Some(repo), Some(proj)) = (repo_id, project_id) {
        format!(
            r#"MATCH (source:GraphNode)-[r:GraphRel]->(target:GraphNode)
WHERE source.repo_id = "{}" AND source.project_id = "{}"
RETURN source.node_id AS source_id, target.node_id AS target_id, r.rel_type AS type
LIMIT {};"#,
            escape_cypher(repo),
            escape_cypher(proj),
            limit
        )
    } else {
        format!(
            r#"MATCH (source:GraphNode)-[r:GraphRel]->(target:GraphNode)
RETURN source.node_id AS source_id, target.node_id AS target_id, r.rel_type AS type
LIMIT {};"#,
            limit
        )
    };

    let sample_rows: Vec<(String, String, String)> = query_rows_3(db, &sample_sql)?;

    if sample_rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // Collect endpoint IDs
    let mut endpoint_ids: Vec<String> = Vec::new();
    for (s, t, _) in &sample_rows {
        if !endpoint_ids.contains(s) {
            endpoint_ids.push(s.clone());
        }
        if !endpoint_ids.contains(t) {
            endpoint_ids.push(t.clone());
        }
    }

    // Step 2: fetch node details
    let nodes = fetch_node_details(db, &endpoint_ids)?;

    // Build sample edges (using ladybug: prefixed IDs)
    let mut edges: Vec<WorkspaceGraphEdge> = sample_rows
        .iter()
        .map(|(s, t, r)| make_edge(&format!("ladybug:{}", s), &format!("ladybug:{}", t), r))
        .collect();

    // Step 3: density boost — all relationships among endpoints
    if endpoint_ids.len() > 1 {
        let id_list = endpoint_ids
            .iter()
            .map(|id| format!("\"{}\"", escape_cypher(id)))
            .collect::<Vec<_>>()
            .join(", ");
        let density_sql = format!(
            r#"MATCH (a:GraphNode)-[r:GraphRel]->(b:GraphNode)
WHERE a.node_id IN [{}] AND b.node_id IN [{}]
RETURN a.node_id AS source_id, b.node_id AS target_id, r.rel_type AS type;"#,
            id_list, id_list
        );

        let density_rows: Vec<(String, String, String)> = query_rows_3(db, &density_sql)?;

        // Only add edges we don't already have
        for (s, t, r) in &density_rows {
            let from = format!("ladybug:{}", s);
            let to = format!("ladybug:{}", t);
            let edge_id = format!("{}--{}--{}", from, r, to);
            if !edges.iter().any(|e| e.edge_id == edge_id) {
                edges.push(make_edge(&from, &to, r));
            }
        }
    }

    Ok((nodes, edges))
}

// ── Fetch node details ───────────────────────────────────────────────

/// Fetch full `WorkspaceGraphNode` records for a list of `node_id`s.
pub fn fetch_node_details(
    db: &LadybugDb,
    node_ids: &[String],
) -> Result<Vec<WorkspaceGraphNode>, String> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }

    let id_list = node_ids
        .iter()
        .map(|id| format!("\"{}\"", escape_cypher(id)))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        r#"MATCH (n:GraphNode)
WHERE n.node_id IN [{}]
RETURN n.node_id AS node_id, n.primary_label AS primary_label, n.name AS name,
       n.path AS path, n.qualified_name AS qualified_name,
       n.repo_id AS repo_id, n.project_id AS project_id, n.text AS text;"#,
        id_list
    );

    query_node_detail_rows(db, &sql)
}

// ── Entity search ────────────────────────────────────────────────────

/// Search for MemoryEntity nodes whose name contains the given keyword.
pub fn search_entities_by_keyword(
    db: &LadybugDb,
    keyword: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let sql = format!(
        r#"MATCH (n:GraphNode)
WHERE n.primary_label = "MemoryEntity" AND n.name CONTAINS "{}"
RETURN n.name AS name LIMIT {};"#,
        escape_cypher(keyword),
        limit
    );

    query_single_string_column(db, &sql)
}

/// Search for MemoryEntity nodes by entity name (exact-ish match).
pub fn search_entities_by_name(
    db: &LadybugDb,
    entity_name: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let sql = format!(
        r#"MATCH (n:GraphNode)
WHERE n.primary_label = "MemoryEntity" AND n.name CONTAINS "{}"
RETURN n.name AS name LIMIT {};"#,
        escape_cypher(entity_name),
        limit
    );

    query_single_string_column(db, &sql)
}

// ── Entity relations ─────────────────────────────────────────────────

/// Fetch relations between MemoryEntity nodes whose names are in the list.
///
/// Returns `(nodes, edges)` where nodes are the entity nodes and edges are
/// the relations between them.
pub fn fetch_entity_relations(
    db: &LadybugDb,
    entity_names: &[String],
    limit: i64,
) -> Result<(Vec<WorkspaceGraphNode>, Vec<WorkspaceGraphEdge>), String> {
    if entity_names.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // Build nodes from names first
    let nodes: Vec<WorkspaceGraphNode> = entity_names
        .iter()
        .map(|name| make_entity_node(name))
        .collect();

    // Query relations between these entities
    let name_list = entity_names
        .iter()
        .map(|n| format!("\"{}\"", escape_cypher(n)))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        r#"MATCH (source:GraphNode)-[r:GraphRel]->(target:GraphNode)
WHERE source.primary_label = "MemoryEntity" AND target.primary_label = "MemoryEntity"
  AND source.name IN [{}] AND target.name IN [{}]
RETURN source.name AS source, target.name AS target, r.rel_type AS type LIMIT {};"#,
        name_list, name_list, limit
    );

    let edges = query_entity_edges(db, &sql)?;

    Ok((nodes, edges))
}

// ── Repos & projects ─────────────────────────────────────────────────

/// List repositories from LadybugDB.
pub fn list_repos(db: &LadybugDb, limit: i64) -> Result<Vec<(String, i64)>, String> {
    let sql = format!(
        r#"MATCH (n:GraphNode)
WHERE n.repo_id IS NOT NULL
RETURN n.repo_id AS repo_id, count(n) AS count
ORDER BY count DESC
LIMIT {};"#,
        limit
    );

    query_repo_project_counts(db, &sql)
}

/// List projects from LadybugDB.
pub fn list_projects(db: &LadybugDb, limit: i64) -> Result<Vec<(String, i64)>, String> {
    let sql = format!(
        r#"MATCH (n:GraphNode)
WHERE n.project_id IS NOT NULL AND n.project_id <> ''
RETURN n.project_id AS project_id, count(n) AS count
ORDER BY count DESC
LIMIT {};"#,
        limit
    );

    query_repo_project_counts(db, &sql)
}

// ── Local data helpers (filesystem, no DB) ───────────────────────────

/// Build local graph nodes/edges for a board and its objects + connectors.
///
/// This is called by `graph_board` to construct the local structure:
/// - seed: `board:{board_id}`
/// - `object:{object_id}` nodes for each board object
/// - `note:{note_id}` nodes for objects with note_id
/// - edges: BOARD_HAS_OBJECT, OBJECT_REFERENCES_NOTE, USER_LINKED
pub fn build_board_local_graph(
    board_id: &str,
    board_title: &str,
    objects: &[am_workspace::model::WorkspaceBoardObject],
    connectors: &[am_workspace::model::WorkspaceConnector],
) -> (Vec<WorkspaceGraphNode>, Vec<WorkspaceGraphEdge>) {
    let mut nodes: Vec<WorkspaceGraphNode> = Vec::new();
    let mut edges: Vec<WorkspaceGraphEdge> = Vec::new();

    // Seed node: the board itself
    nodes.push(WorkspaceGraphNode {
        node_id: format!("board:{}", board_id),
        node_type: "board".to_string(),
        title: board_title.to_string(),
        subtitle: None,
        metadata: json!({"source": "local", "board_id": board_id}),
    });

    // Object nodes and BOARD_HAS_OBJECT edges
    for obj in objects {
        let obj_node_id = format!("object:{}", obj.object_id);
        nodes.push(WorkspaceGraphNode {
            node_id: obj_node_id.clone(),
            node_type: obj.object_type.clone(),
            title: obj.title.clone().unwrap_or_else(|| obj.object_id.clone()),
            subtitle: obj.summary.clone(),
            metadata: json!({
                "source": "local",
                "object_type": obj.object_type,
                "note_id": obj.note_id,
            }),
        });

        edges.push(WorkspaceGraphEdge {
            edge_id: format!("board:{}--BOARD_HAS_OBJECT--{}", board_id, obj_node_id),
            from_node_id: format!("board:{}", board_id),
            to_node_id: obj_node_id.clone(),
            relation_type: "BOARD_HAS_OBJECT".to_string(),
            label: None,
            metadata: json!({"source": "local"}),
        });

        // If object references a note, add note node + edge
        if let Some(ref note_id) = obj.note_id {
            let note_node_id = format!("note:{}", note_id);
            if !nodes.iter().any(|n| n.node_id == note_node_id) {
                nodes.push(WorkspaceGraphNode {
                    node_id: note_node_id.clone(),
                    node_type: "note".to_string(),
                    title: obj.title.clone().unwrap_or_else(|| note_id.clone()),
                    subtitle: None,
                    metadata: json!({"source": "local", "note_id": note_id}),
                });
            }

            edges.push(WorkspaceGraphEdge {
                edge_id: format!(
                    "{}--OBJECT_REFERENCES_NOTE--{}",
                    obj_node_id, note_node_id
                ),
                from_node_id: obj_node_id.clone(),
                to_node_id: note_node_id,
                relation_type: "OBJECT_REFERENCES_NOTE".to_string(),
                label: None,
                metadata: json!({"source": "local"}),
            });
        }
    }

    // Connector edges (USER_LINKED)
    for conn in connectors {
        let from_obj = format!("object:{}", conn.from_object_id);
        let to_obj = format!("object:{}", conn.to_object_id);
        edges.push(WorkspaceGraphEdge {
            edge_id: format!(
                "{}--USER_LINKED--{}",
                from_obj, to_obj
            ),
            from_node_id: from_obj,
            to_node_id: to_obj,
            relation_type: "USER_LINKED".to_string(),
            label: conn.custom_label.clone(),
            metadata: json!({
                "source": "local",
                "relation_intent": conn.relation_intent,
            }),
        });
    }

    (nodes, edges)
}

/// Build local graph nodes/edges for a note and boards that contain it.
///
/// Called by `graph_note` to construct the local neighbourhood.
pub fn build_note_local_graph(
    note_id: &str,
    note_title: &str,
    boards: &[am_workspace::model::WorkspaceBoard],
) -> (Vec<WorkspaceGraphNode>, Vec<WorkspaceGraphEdge>) {
    let mut nodes: Vec<WorkspaceGraphNode> = Vec::new();
    let mut edges: Vec<WorkspaceGraphEdge> = Vec::new();

    // Seed node: the note
    nodes.push(WorkspaceGraphNode {
        node_id: format!("note:{}", note_id),
        node_type: "note".to_string(),
        title: note_title.to_string(),
        subtitle: None,
        metadata: json!({"source": "local", "note_id": note_id}),
    });

    // Find boards containing this note_id in their objects
    for board in boards {
        let board_node_id = format!("board:{}", board.board_id);
        let mut board_has_note = false;

        // Add board node if not already present
        if !nodes.iter().any(|n| n.node_id == board_node_id) {
            nodes.push(WorkspaceGraphNode {
                node_id: board_node_id.clone(),
                node_type: "board".to_string(),
                title: board.title.clone(),
                subtitle: None,
                metadata: json!({
                    "source": "local",
                    "board_id": board.board_id,
                }),
            });
        }

        // Check each object for note_id match
        for obj in &board.objects {
            if obj.note_id.as_ref() == Some(&note_id.to_string()) {
                board_has_note = true;

                let obj_node_id = format!("object:{}", obj.object_id);
                if !nodes.iter().any(|n| n.node_id == obj_node_id) {
                    nodes.push(WorkspaceGraphNode {
                        node_id: obj_node_id.clone(),
                        node_type: obj.object_type.clone(),
                        title: obj.title.clone().unwrap_or_else(|| obj.object_id.clone()),
                        subtitle: obj.summary.clone(),
                        metadata: json!({"source": "local", "object_type": obj.object_type}),
                    });
                }

                // board -> object edge
                edges.push(WorkspaceGraphEdge {
                    edge_id: format!(
                        "{}--BOARD_HAS_OBJECT--{}",
                        board_node_id, obj_node_id
                    ),
                    from_node_id: board_node_id.clone(),
                    to_node_id: obj_node_id.clone(),
                    relation_type: "BOARD_HAS_OBJECT".to_string(),
                    label: None,
                    metadata: json!({"source": "local"}),
                });

                // object -> note edge
                let note_node_id = format!("note:{}", note_id);
                edges.push(WorkspaceGraphEdge {
                    edge_id: format!(
                        "{}--OBJECT_REFERENCES_NOTE--{}",
                        obj_node_id, note_node_id
                    ),
                    from_node_id: obj_node_id,
                    to_node_id: note_node_id,
                    relation_type: "OBJECT_REFERENCES_NOTE".to_string(),
                    label: None,
                    metadata: json!({"source": "local"}),
                });
            }
        }

        // If board has this note, add a direct board->note edge for navigability
        if board_has_note {
            let note_node_id = format!("note:{}", note_id);
            edges.push(WorkspaceGraphEdge {
                edge_id: format!(
                    "{}--CONTAINS_NOTE--{}",
                    board_node_id, note_node_id
                ),
                from_node_id: board_node_id,
                to_node_id: note_node_id,
                relation_type: "CONTAINS_NOTE".to_string(),
                label: None,
                metadata: json!({"source": "local"}),
            });
        }
    }

    (nodes, edges)
}

/// Run an ad-hoc read-only query for the operator shell; returns stringified cells.
pub fn run_adhoc_query(
    db: &LadybugDb,
    sql: &str,
    max_rows: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>, bool), String> {
    #[cfg(feature = "ladybug")]
    {
        let conn = lbug::Connection::new(&db.db).map_err(|e| e.to_string())?;
        let mut columns: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut truncated = false;
        for (idx, row) in conn
            .query(sql)
            .map_err(|e| e.to_string())?
            .into_iter()
            .enumerate()
        {
            if idx >= max_rows {
                truncated = true;
                break;
            }
            if columns.is_empty() {
                columns = (0..row.len())
                    .map(|i| format!("col_{i}"))
                    .collect();
            }
            let mut cells = Vec::with_capacity(row.len());
            for v in row {
                cells.push(value_as_required_string(&v)?);
            }
            rows.push(cells);
        }
        Ok((columns, rows, truncated))
    }
    #[cfg(not(feature = "ladybug"))]
    {
        let _ = (db, sql, max_rows);
        Ok((vec![], vec![], false))
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_cypher() {
        assert_eq!(escape_cypher(r#"hello"world"#), r#"hello\"world"#);
        assert_eq!(escape_cypher(r#"a\b"#), r#"a\\b"#);
        assert_eq!(escape_cypher("plain"), "plain");
    }

    #[test]
    fn test_extract_keywords() {
        let title = "The Quick Brown Fox Jumps Over The Lazy Dog";
        let kw = extract_keywords(title, 3);
        assert_eq!(kw, vec!["quick", "brown", "fox"]);
    }

    #[test]
    fn test_extract_keywords_filters_stop_words() {
        let title = "The Note Board For Testing";
        let kw = extract_keywords(title, 5);
        assert!(!kw.contains(&"the".to_string()));
        assert!(!kw.contains(&"note".to_string()));
        assert!(!kw.contains(&"board".to_string()));
        assert!(!kw.contains(&"for".to_string()));
        assert!(kw.contains(&"testing".to_string()));
    }

    #[test]
    fn test_extract_keywords_short_words() {
        let title = "A B C D elephant";
        let kw = extract_keywords(title, 5);
        assert_eq!(kw, vec!["elephant"]);
    }

    #[test]
    fn test_extract_keywords_respects_max() {
        let title = "apple banana cherry date elderberry fig";
        let kw = extract_keywords(title, 3);
        assert_eq!(kw.len(), 3);
        assert_eq!(kw, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_build_board_local_graph() {
        let objects = vec![
            am_workspace::model::WorkspaceBoardObject {
                object_id: "obj-1".to_string(),
                board_id: "board-1".to_string(),
                workspace_id: "ws-1".to_string(),
                project_id: None,
                object_type: "note".to_string(),
                title: Some("My Note".to_string()),
                summary: None,
                note_id: Some("note-abc".to_string()),
                asset_id: None,
                artifact_id: None,
                graph_entity_name: None,
                graph_source_id: None,
                tags: Vec::new(),
                ingest_eligible: true,
                locked: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ];
        let connectors = vec![am_workspace::model::WorkspaceConnector {
            connector_id: "conn-1".to_string(),
            board_id: "board-1".to_string(),
            workspace_id: "ws-1".to_string(),
            project_id: None,
            from_object_id: "obj-1".to_string(),
            to_object_id: "obj-2".to_string(),
            connector_type: "relation".to_string(),
            relation_intent: "depends_on".to_string(),
            custom_label: Some("uses".to_string()),
            user_authored_summary: None,
            ingest_eligible: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }];

        let (nodes, edges) = build_board_local_graph("board-1", "My Board", &objects, &connectors);

        assert_eq!(nodes.len(), 3); // board + object + note
        assert!(nodes.iter().any(|n| n.node_id == "board:board-1"));
        assert!(nodes.iter().any(|n| n.node_id == "object:obj-1"));
        assert!(nodes.iter().any(|n| n.node_id == "note:note-abc"));

        assert!(!edges.is_empty());
        assert!(edges.iter().any(|e| e.relation_type == "BOARD_HAS_OBJECT"));
        assert!(edges
            .iter()
            .any(|e| e.relation_type == "OBJECT_REFERENCES_NOTE"));
        assert!(edges.iter().any(|e| e.relation_type == "USER_LINKED"));
    }

    #[test]
    fn test_build_note_local_graph() {
        let boards = vec![am_workspace::model::WorkspaceBoard {
            board_id: "board-1".to_string(),
            workspace_id: "ws-1".to_string(),
            project_id: None,
            title: "Test Board".to_string(),
            description: None,
            tags: Vec::new(),
            board_type: "canvas".to_string(),
            board_state: "active".to_string(),
            tldraw_document: serde_json::json!({}),
            objects: vec![am_workspace::model::WorkspaceBoardObject {
                object_id: "obj-1".to_string(),
                board_id: "board-1".to_string(),
                workspace_id: "ws-1".to_string(),
                project_id: None,
                object_type: "note".to_string(),
                title: Some("My Note Object".to_string()),
                summary: None,
                note_id: Some("note-abc".to_string()),
                asset_id: None,
                artifact_id: None,
                graph_entity_name: None,
                graph_source_id: None,
                tags: Vec::new(),
                ingest_eligible: true,
                locked: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }],
            connectors: Vec::new(),
            object_count: 1,
            connector_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            ingested_at: None,
            graph_status: "not_ingested".to_string(),
        }];

        let (nodes, edges) = build_note_local_graph("note-abc", "My Note", &boards);

        assert!(nodes.iter().any(|n| n.node_id == "note:note-abc"));
        assert!(nodes.iter().any(|n| n.node_id == "board:board-1"));
        assert!(nodes.iter().any(|n| n.node_id == "object:obj-1"));
        assert!(edges
            .iter()
            .any(|e| e.relation_type == "BOARD_HAS_OBJECT"));
        assert!(edges
            .iter()
            .any(|e| e.relation_type == "OBJECT_REFERENCES_NOTE"));
    }
}
