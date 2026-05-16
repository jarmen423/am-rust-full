//! Native Ladybug writes for Agentic Memory `CODE_SCHEMA` (`ladybug/schema.py`).
//!
//! `Chunk` rows carrying `embedding FLOAT[N]` mirror `LadybugNativeCodeWriter` / `_replace_vector_node`:
//! **`DETACH DELETE` before `CREATE`** when revisiting the same deterministic `chunk:` id.

use lbug::{Connection, LogicalType, SystemConfig, Value};

use super::schema_ddl::{CREATE_CODE_VECTOR_INDEX_SQL, INSTALL_VECTOR_SQL};

const MAX_STORED_CODE_TEXT_CHARS: usize = 12_000;

fn esc_lit(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn now_rfc3339_secs() -> String {
    use chrono::SecondsFormat;
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn stored_visible_code(text_for_embedding: &str) -> String {
    let truncated: String = text_for_embedding
        .chars()
        .take(MAX_STORED_CODE_TEXT_CHARS)
        .collect();
    truncated.replace(';', ",")
}

/// Hard cap embedding payload UTF-8 size before calling Jina.
pub(crate) fn trim_for_embedding(text_for_embedding: &str) -> String {
    text_for_embedding.chars().take(120_000).collect()
}

pub(crate) fn open_writable_database(path: &str) -> Result<lbug::Database, String> {
    lbug::Database::new(path, SystemConfig::default()).map_err(|e| format!("Ladybug Database::new: {e}"))
}

pub(crate) fn init_code_schema(conn: &Connection<'_>, ddls: &[String]) -> Result<(), String> {
    for ddl in ddls {
        if let Err(e) = conn.query(ddl.as_str()) {
            let msg = e.to_string();
            if msg.contains("already exists") {
                continue;
            }
            return Err(format!("DDL failed: {ddl}\n{e}"));
        }
    }

    conn.query(INSTALL_VECTOR_SQL)
        .map(|_| ())
        .map_err(|e| format!("INSTALL VECTOR failed: {e}"))?;
    exec_ignore_vector_dupe(conn)
}

fn exec_ignore_vector_dupe(conn: &Connection<'_>) -> Result<(), String> {
    if let Err(e) = conn.query(CREATE_CODE_VECTOR_INDEX_SQL).map(|_| ()) {
        let msg = e.to_string();
        if msg.contains("code_chunk_embedding_index already exists")
            || msg.contains("Index code_chunk_embedding_index already exists")
        {
            return Ok(());
        }
        return Err(format!("vector index create failed unexpectedly: {e}"));
    }
    Ok(())
}

pub(crate) fn delete_file_derivatives(conn: &Connection<'_>, repo_id: &str, rel_path: &str) -> Result<(), String> {
    let r = esc_lit(repo_id);
    let p = esc_lit(rel_path);
    conn.query(&format!(
        "MATCH (ch:Chunk) WHERE ch.repo_id = \"{r}\" AND ch.path = \"{p}\" DETACH DELETE ch;\
         MATCH (fn:Function) WHERE fn.repo_id = \"{r}\" AND fn.path = \"{p}\" DETACH DELETE fn;\
         MATCH (c:Class) WHERE c.repo_id = \"{r}\" AND c.path = \"{p}\" DETACH DELETE c;"
    ))
    .map(|_| ())
    .map_err(|e| format!("delete_file_derivatives failed: {e}"))
}

fn detach_chunk_any(conn: &Connection<'_>, chunk_id: &str) -> Result<(), String> {
    let cid = esc_lit(chunk_id);
    conn.query(&format!(
        "MATCH (ch:Chunk) WHERE ch.id = \"{cid}\" DETACH DELETE ch;"
    ))
        .map(|_| ())
        .map_err(|e| format!("drop chunk failed: {e}"))
}

pub(crate) fn insert_chunk_dense(
    conn: &Connection<'_>,
    id: &str,
    repo_id: &str,
    rel_path: &str,
    name: &str,
    embedding: &[f32],
    props_json_compact: &str,
    embed_source_trimmed_for_id: &str,
) -> Result<(), String> {
    detach_chunk_any(conn, id)?;

    let text_stored = stored_visible_code(embed_source_trimmed_for_id);
    let emb_vals: Vec<Value> = embedding.iter().map(|x| Value::Float(*x)).collect();
    let emb = Value::List(LogicalType::Float, emb_vals);

    let mut pstmt = conn
        .prepare(
            "CREATE (n:Chunk {\n\
  id: $id,\n\
  repo_id: $repo_id,\n\
  path: $path,\n\
  name: $name,\n\
  text: $text,\n\
  embedding: $embedding,\n\
  properties_json: $properties_json\n\
});",
        )
        .map_err(|e| format!("prepare Chunk insert failed: {e}"))?;

    conn.execute(
        &mut pstmt,
        vec![
            ("id", id.into()),
            ("repo_id", repo_id.into()),
            ("path", rel_path.into()),
            ("name", name.into()),
            ("text", text_stored.into()),
            ("embedding", emb),
            ("properties_json", props_json_compact.into()),
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("Chunk insert execute failed for {id}: {e}"))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_code_document_pending(
    conn: &Connection<'_>,
    id: &str,
    repo_id: &str,
    rel_path: &str,
    name: &str,
    ohash: &str,
    repo_root: &str,
) -> Result<(), String> {
    let pk_esc = esc_lit(id);
    let probe = conn
        .query(&format!(
            "MATCH (d:CodeDocument) WHERE d.id = \"{pk_esc}\" RETURN 1 LIMIT 1;"
        ))
        .map_err(|e| format!("CodeDocument existence probe failed: {e}"))?;
    let exists = probe.into_iter().next().is_some();

    let meta = esc_lit(r#"{"storage_mode":"native","ingest":"jina-ladybug-repo-index"}"#);
    let updated = esc_lit(&now_rfc3339_secs());

    if exists {
        conn.query(&format!(
            "MATCH (d:CodeDocument) WHERE d.id = \"{}\" \
         SET d.repo_id = \"{}\", d.path = \"{}\", d.name = \"{}\", d.source_hash = \"\", \
             d.pending_source_hash = \"{}\", d.index_status = \"pending\", d.repo_root = \"{}\", \
             d.metadata_json = \"{}\", d.updated_at = \"{}\";",
            esc_lit(id),
            esc_lit(repo_id),
            esc_lit(rel_path),
            esc_lit(name),
            esc_lit(ohash),
            esc_lit(repo_root),
            meta,
            updated,
        ))
        .map(|_| ())
        .map_err(|e| format!("CodeDocument pending update failed: {e}"))
    } else {
    conn.query(&format!(
        "CREATE (d:CodeDocument {{ \
          id: \"{}\", repo_id: \"{}\", path: \"{}\", name: \"{}\", \
          source_hash: \"\", pending_source_hash: \"{}\", index_status: \"pending\", \
          repo_root: \"{}\", metadata_json: \"{}\", updated_at: \"{}\" \
        }});",
        esc_lit(id),
        esc_lit(repo_id),
        esc_lit(rel_path),
        esc_lit(name),
        esc_lit(ohash),
        esc_lit(repo_root),
        meta,
        updated,
    ))
    .map(|_| ())
    .map_err(|e| format!("CodeDocument create failed: {e}"))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_file_pending(
    conn: &Connection<'_>,
    id: &str,
    repo_id: &str,
    rel_path: &str,
    name: &str,
    ohash: &str,
    repo_root: &str,
) -> Result<(), String> {
    let pk_esc = esc_lit(id);
    let probe = conn
        .query(&format!(
            "MATCH (f:File) WHERE f.id = \"{pk_esc}\" RETURN 1 LIMIT 1;"
        ))
        .map_err(|e| format!("File existence probe failed: {e}"))?;
    let exists = probe.into_iter().next().is_some();

    let props = serde_json::json!({
        "ohash": ohash,
        "pending_ohash": ohash,
        "index_status": "pending",
        "repo_root": repo_root,
        "last_attempted_at": now_rfc3339_secs(),
        "storage_mode": "native",
    })
    .to_string();
    let props_esc = esc_lit(&props);

    if exists {
        conn.query(&format!(
            "MATCH (f:File) WHERE f.id = \"{}\" \
         SET f.repo_id = \"{}\", f.path = \"{}\", f.name = \"{}\", f.text = \"{}\", f.properties_json = \"{}\";",
            esc_lit(id),
            esc_lit(repo_id),
            esc_lit(rel_path),
            esc_lit(name),
            esc_lit(rel_path),
            props_esc,
        ))
        .map(|_| ())
        .map_err(|e| format!("File pending update failed: {e}"))
    } else {
        conn.query(&format!(
            "CREATE (f:File {{ \
          id: \"{}\", repo_id: \"{}\", path: \"{}\", name: \"{}\", text: \"{}\", properties_json: \"{}\" \
        }});",
            esc_lit(id),
            esc_lit(repo_id),
            esc_lit(rel_path),
            esc_lit(name),
            esc_lit(rel_path),
            props_esc,
        ))
        .map(|_| ())
        .map_err(|e| format!("File create failed: {e}"))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn mark_file_and_document_complete(
    conn: &Connection<'_>,
    doc_id: &str,
    file_pk: &str,
    repo_id: &str,
    rel_path: &str,
    name: &str,
    ohash: &str,
    repo_root: &str,
    jina_fingerprint: &str,
) -> Result<(), String> {
    let meta = esc_lit(r#"{"storage_mode":"native","ingest":"jina-ladybug-repo-index"}"#);
    let updated = esc_lit(&now_rfc3339_secs());
    conn.query(&format!(
        "MATCH (d:CodeDocument) WHERE d.id = \"{}\" \
         SET d.repo_id = \"{}\", d.path = \"{}\", d.name = \"{}\", d.source_hash = \"{}\", \
             d.pending_source_hash = \"\", d.index_status = \"complete\", d.repo_root = \"{}\", \
             d.metadata_json = \"{}\", d.updated_at = \"{}\";",
        esc_lit(doc_id),
        esc_lit(repo_id),
        esc_lit(rel_path),
        esc_lit(name),
        esc_lit(ohash),
        esc_lit(repo_root),
        meta,
        updated,
    ))
    .map(|_| ())
    .map_err(|e| format!("CodeDocument completion failed: {e}"))?;

    let props = serde_json::json!({
        "ohash": ohash,
        "pending_ohash": "",
        "index_status": "complete",
        "repo_root": repo_root,
        "last_indexed_at": now_rfc3339_secs(),
        "storage_mode": "native",
        "jina_fingerprint": jina_fingerprint,
    })
    .to_string();
    let props_esc = esc_lit(&props);
    conn.query(&format!(
        "MATCH (f:File) WHERE f.id = \"{}\" \
         SET f.repo_id = \"{}\", f.path = \"{}\", f.name = \"{}\", f.text = \"{}\", f.properties_json = \"{}\";",
        esc_lit(file_pk),
        esc_lit(repo_id),
        esc_lit(rel_path),
        esc_lit(name),
        esc_lit(rel_path),
        props_esc,
    ))
    .map(|_| ())
    .map_err(|e| format!("File completion failed: {e}"))
}

/// Runs before we attempt to skip ingest: pulls `CodeDocument` hash/status plus **`jina_fingerprint`**
/// from **`File.properties_json`** (`None` fingerprint ⇒ caller must **not** short‑circuit ingest).
pub(crate) fn fetch_indexed_document_snapshot(
    conn: &Connection<'_>,
    doc_id: &str,
    file_pk: &str,
) -> Result<Option<(String, String, Option<String>)>, String> {
    let de = esc_lit(doc_id);
    let fe = esc_lit(file_pk);
    let stmt = format!(
        "MATCH (d:CodeDocument), (f:File) WHERE d.id = \"{de}\" AND f.id = \"{fe}\" \
         RETURN d.source_hash, d.index_status, f.properties_json LIMIT 1;",
    );
    let mut qr = conn
        .query(&stmt)
        .map_err(|e| format!("indexed snapshot probe failed: {e}"))?;
    let Some(row) = qr.next() else {
        return Ok(None);
    };
    if row.len() < 3 {
        return Ok(None);
    }
    let sh = match &row[0] {
        Value::String(s) => s.clone(),
        _ => return Ok(None),
    };
    let status = match &row[1] {
        Value::String(s) => s.clone(),
        _ => return Ok(None),
    };
    let fp = match &row[2] {
        Value::String(props_json) => serde_json::from_str::<serde_json::Value>(props_json).ok(),
        _ => None,
    }
    .and_then(|v| {
        v.get("jina_fingerprint")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
    });
    Ok(Some((sh, status, fp)))
}

pub(crate) fn jina_fingerprint_for_run(model: &str, task: &str, dimensions: usize) -> String {
    format!("{model}|{task}|{dimensions}")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_function_node(
    conn: &Connection<'_>,
    fid: &str,
    repo_id: &str,
    rel_path: &str,
    name: &str,
    qualified_name: &str,
    signature: &str,
    text_for_embedding: &str,
    name_line: usize,
) -> Result<(), String> {
    let text_stored = stored_visible_code(text_for_embedding);
    let props = serde_json::json!({
        "code_hash": crate::repo_jina_lb::ids::sha256_hex_utf8(text_for_embedding),
        "signature": signature,
        "name_line": name_line,
        "text_truncated": text_for_embedding.chars().count() > MAX_STORED_CODE_TEXT_CHARS,
        "embedding_model": "jina-embeddings-v4",
        "embedding_task": "code.passage",
        "storage_mode": "native",
    })
    .to_string();

    match_or_create_node(conn, "Function", fid, &[
        ("repo_id", repo_id),
        ("path", rel_path),
        ("name", name),
        ("qualified_name", qualified_name),
        ("signature", signature),
        ("docstring", ""),
        ("text", &text_stored),
        ("properties_json", &props),
    ])
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_class_node(
    conn: &Connection<'_>,
    cid: &str,
    repo_id: &str,
    rel_path: &str,
    name: &str,
    qualified_name: &str,
    signature: &str,
    text_for_embedding: &str,
) -> Result<(), String> {
    let text_stored = stored_visible_code(text_for_embedding);
    let props = serde_json::json!({
        "code_hash": crate::repo_jina_lb::ids::sha256_hex_utf8(text_for_embedding),
        "signature": signature,
        "text_truncated": text_for_embedding.chars().count() > MAX_STORED_CODE_TEXT_CHARS,
        "embedding_model": "jina-embeddings-v4",
        "embedding_task": "code.passage",
        "storage_mode": "native",
    })
    .to_string();

    match_or_create_node(conn, "Class", cid, &[
        ("repo_id", repo_id),
        ("path", rel_path),
        ("name", name),
        ("qualified_name", qualified_name),
        ("docstring", ""),
        ("text", &text_stored),
        ("properties_json", &props),
    ])
}

fn match_or_create_node(
    conn: &Connection<'_>,
    label: &str,
    pk: &str,
    fields: &[(&str, &str)],
) -> Result<(), String> {
    let pk_esc = esc_lit(pk);

    let mut hit = conn
        .query(&format!(
            "MATCH (n:{label}) WHERE n.id = \"{pk_esc}\" RETURN 1 LIMIT 1;"
        ))
        .map_err(|e| format!("{label} existence probe failed for {pk}: {e}"))?;
    let exists = hit.next().is_some();

    if exists {
        let mut assigns = Vec::new();
        for (col, val) in fields {
            assigns.push(format!("n.{col} = \"{}\"", esc_lit(val)));
        }
        let stmt = format!(
            "MATCH (n:{label}) WHERE n.id = \"{pk_esc}\" SET {};",
            assigns.join(", ")
        );
        conn.query(&stmt)
            .map(|_| ())
            .map_err(|e| format!("{label} UPDATE failed for {pk}: {e}"))
    } else {
        let mut prop_lines = Vec::new();
        prop_lines.push(format!("id: \"{pk_esc}\""));
        for (col, val) in fields {
            prop_lines.push(format!("{col}: \"{}\"", esc_lit(val)));
        }
        let stmt = format!("CREATE (n:{label} {{ {} }});", prop_lines.join(", "));
        conn.query(&stmt)
            .map(|_| ())
            .map_err(|e| format!("{label} CREATE failed for {pk}: {e}"))
    }
}

pub(crate) fn create_rel_defines(conn: &Connection<'_>, file_id: &str, target_label: &str, target_pk: &str) -> Result<(), String> {
    let mut hit = conn
        .query(&format!(
            "MATCH (f:File)-[:DEFINES]->(t:{target_label}) WHERE f.id=\"{}\" AND t.id=\"{}\" RETURN 1 LIMIT 1;",
            esc_lit(file_id),
            esc_lit(target_pk),
        ))
        .map_err(|e| format!("defines probe failed: {e}"))?;
    if hit.next().is_some() {
        return Ok(());
    }
    conn.query(&format!(
        "MATCH (f:File), (t:{target_label}) WHERE f.id=\"{}\" AND t.id=\"{}\" CREATE (f)-[:DEFINES {{ properties_json: \"{{}}\" }}]->(t);",
        esc_lit(file_id),
        esc_lit(target_pk),
    ))
    .map(|_| ())
    .map_err(|e| format!("CREATE DEFINES failed: {e}"))
}

pub(crate) fn create_rel_describes(
    conn: &Connection<'_>,
    chunk_id: &str,
    target_label: &str,
    target_pk: &str,
) -> Result<(), String> {
    let mut hit = conn
        .query(&format!(
            "MATCH (c:Chunk)-[:DESCRIBES]->(t:{target_label}) WHERE c.id=\"{}\" AND t.id=\"{}\" RETURN 1 LIMIT 1;",
            esc_lit(chunk_id),
            esc_lit(target_pk),
        ))
        .map_err(|e| format!("describes probe failed: {e}"))?;
    if hit.next().is_some() {
        return Ok(());
    }
    conn.query(&format!(
        "MATCH (c:Chunk), (t:{target_label}) WHERE c.id=\"{}\" AND t.id=\"{}\" CREATE (c)-[:DESCRIBES {{ properties_json: \"{{}}\" }}]->(t);",
        esc_lit(chunk_id),
        esc_lit(target_pk),
    ))
    .map(|_| ())
    .map_err(|e| format!("CREATE DESCRIBES failed: {e}"))
}

/// Idempotent **`CALLS`** (`Function → Function`) used by rust-analyzer SCIP ingest.
///
/// SCIP merges both “semantic call” reads and finer-grained usages into `Relationship.is_reference`;
/// until we tighten heuristics, “reference-from-callable-symbol” surfaces as **`CALLS`**.
/// Returns **`Ok(false)`** if the edge already existed (idempotent).
pub(crate) fn create_rel_calls(conn: &Connection<'_>, caller_id: &str, callee_id: &str) -> Result<bool, String> {
    if caller_id == callee_id {
        return Ok(false);
    }

    let mut hit = conn
        .query(&format!(
            "MATCH (caller:Function)-[:CALLS]->(callee:Function) WHERE caller.id=\"{}\" AND callee.id=\"{}\" RETURN 1 LIMIT 1;",
            esc_lit(caller_id),
            esc_lit(callee_id),
        ))
        .map_err(|e| format!("CALLS probe failed: {e}"))?;
    if hit.next().is_some() {
        return Ok(false);
    }

    conn.query(&format!(
        "MATCH (caller:Function), (callee:Function) WHERE caller.id=\"{}\" AND callee.id=\"{}\" \
         CREATE (caller)-[:CALLS {{ properties_json: \"{{}}\" }}]->(callee);",
        esc_lit(caller_id),
        esc_lit(callee_id),
    ))
    .map(|_| ())
    .map_err(|e| format!("CREATE CALLS failed: {e}"))?;
    Ok(true)
}
