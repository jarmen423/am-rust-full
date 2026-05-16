//! **Jina `jina-embeddings-v4` → Ladybug native code graph**
//!
//! This module implements a small ingestion pipeline aligned with Agentic Memory’s
//! `CODE_SCHEMA` in `agentic_memory.ladybug.schema` (Python): `CodeDocument`, `File`,
//! `Function` / `Class`, `Chunk` with **`FLOAT[N]` embeddings** (`N` configurable,
//! defaults to **`2048`**, Jina v4 dense max), plus `DEFINES` and `DESCRIBES` edges.
//!
//! ## Operational flow (`jina-ladybug-repo-index` binary)
//! 1. Walk the repository with `ignore` (respect `.gitignore`).
//! 2. For each source file (`rs`, `py`, `js`/`jsx`, `ts`/`tsx`), parse tree-sitter
//!    units (`parse`).
//! 3. Embed unit texts via Jina `code.passage` (`jina`).
//! 4. Write nodes/edges via prepared `Chunk` inserts and interpolated Cypher for
//!    graph labels (`ladybug_writes`) — **`DETACH DELETE` old `Chunk` before `CREATE`**
//!    when revisiting the same deterministic `chunk:` id.
//! 5. Optionally load **rust-analyzer `index.scip`** and write **`CALLS`** edges
//!    (`scip_calls`), aligned via definition-line anchors captured during step 2.

pub mod calls_registry;
pub mod ids;
pub mod jina;
pub mod ladybug_writes;
pub mod parse;
pub mod schema_ddl;
pub mod scip_calls;

use calls_registry::FunctionAnchorRegistry;
use std::path::{Path, PathBuf};

use clap::Parser;

use scip_calls::{ingest_scip_calls, resolve_scip_index_path};

use ladybug_writes::{
    create_rel_defines, create_rel_describes, delete_file_derivatives,
    insert_chunk_dense, init_code_schema, mark_file_and_document_complete, open_writable_database,
    trim_for_embedding, upsert_code_document_pending, upsert_file_pending,
    upsert_class_node, upsert_function_node,
};
use parse::{extract_units, language_for_extension, rel_path_posix};

/// CLI surface for [`main_cli`] (invoked by `src/bin/jina_ladybug_repo_index.rs`).
#[derive(Parser, Debug)]
#[command(
    name = "jina-ladybug-repo-index",
    about = "Embed a repo with Jina embeddings v4 and write Agentic Memory native Ladybug code nodes"
)]
pub struct Cli {
    /// Repository root scanned for source files (gitignore-aware).
    #[arg(long, default_value = ".", value_name = "DIR")]
    pub repo: PathBuf,

    /// Path to writable Ladybug `.lbug` database.
    #[arg(long, value_name = "PATH.lbug")]
    pub db: PathBuf,

    /// Tenant/repo id persisted on every row (`repo_id`).
    #[arg(long)]
    pub repo_id: String,

    /// Create code tables / vector extension / index if missing.
    ///
    /// **Embedding dimension** (`--dimensions`) becomes part of the `Chunk.embedding` DDL;
    /// changing it later requires a new database or migration.
    #[arg(long)]
    pub init_schema: bool,

    /// Dense vector length for `FLOAT[N]` Chunk column and Jina `"dimensions"` request field.
    #[arg(long, default_value_t = 2048)]
    pub dimensions: u32,

    /// Max embedding units passed to Jina in one HTTPS request body.
    #[arg(long, default_value_t = 32)]
    pub batch_units: usize,

    #[arg(long, default_value = "jina-embeddings-v4")]
    pub jina_model: String,

    #[arg(long, default_value = "code.passage")]
    pub jina_task: String,

    /// Bearer token (`Authorization: Bearer ...`). Overrides `JINA_API_KEY`.
    #[arg(long, env = "JINA_API_KEY", value_name = "TOKEN")]
    pub jina_api_key: Option<String>,

    /// Path to protobuf **`index.scip`** (rust-analyzer). Overrides auto-discovery under `--repo`.
    #[arg(long, value_name = "PATH")]
    pub scip: Option<PathBuf>,
}

/// Parsed CLI entry point for tooling and tests.
///
/// Canonical usage (standalone crate):\
/// `cargo run --bin jina-ladybug-repo-index -- ...`\
/// When embedded in `am-workspace`: `cargo run -p am-workspace --features jina-ladybug-index --bin jina-ladybug-repo-index -- ...`
pub fn main_cli() -> Result<(), String> {
    let cli = Cli::parse();
    let api_key = cli
        .jina_api_key
        .or_else(|| std::env::var("JINA_API_KEY").ok())
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err("Missing Jina API credential: pass --jina-api-key or set JINA_API_KEY".into());
    }

    let dims = cli.dimensions as usize;
    if dims == 0 {
        return Err("--dimensions must be positive".into());
    }
    if dims > schema_ddl::JINA_EMBED_DIM_V4_MAX {
        return Err(format!(
            "--dimensions ({dims}) exceeds Jina v4 dense max {} for this ingest profile.",
            schema_ddl::JINA_EMBED_DIM_V4_MAX
        ));
    }

    let repo_canon = cli
        .repo
        .canonicalize()
        .map_err(|e| format!("repo path {:?}: {e}", cli.repo))?;

    let db_str = cli.db.to_string_lossy().to_string();
    let repo_root_disp = posix_path_display(&repo_canon);

    let db = open_writable_database(&db_str)?;
    let conn = lbug::Connection::new(&db).map_err(|e| format!("Ladybug Connection::new: {e}"))?;

    if cli.init_schema {
        init_code_schema(&conn, &schema_ddl::native_code_ddl(dims))?;
    }

    let http = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client build: {e}"))?;

    let jopts = jina::JinaEmbedOpts {
        api_key: api_key.trim(),
        model: cli.jina_model.as_str(),
        task: cli.jina_task.as_str(),
        dimensions: cli.dimensions,
        truncate: true,
        late_chunking: false,
        normalized: true,
        return_multivector: false,
        ..Default::default()
    };

    let mut fn_registry = FunctionAnchorRegistry::default();

    let walker = ignore::WalkBuilder::new(&repo_canon).hidden(false).git_ignore(true).build();
    for entry in walker {
        let entry = entry.map_err(|e| format!("walk: {e}"))?;
        if !entry
            .file_type()
            .map(|ft| ft.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        let Some(ext_os) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some(lang) = language_for_extension(ext_os) else {
            continue;
        };

        let rel = rel_path_posix(&repo_canon, path)?;
        let stem = Path::new(&rel)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.clone());

        match index_one_file(
            &conn,
            &http,
            &jopts,
            &cli.repo_id,
            &repo_canon,
            &repo_root_disp,
            &rel,
            stem,
            lang,
            dims,
            cli.batch_units,
            &mut fn_registry,
        ) {
            Ok(()) => eprintln!("ok  {rel}"),
            Err(err) => eprintln!("ERR {rel}: {err}"),
        }
    }

    if let Some(idx_path) = resolve_scip_index_path(&repo_canon, cli.scip.as_deref()) {
        eprintln!(
            "scip: using index {} with {} anchored Function rows from tree-sitter",
            idx_path.display(),
            fn_registry.len()
        );
        match ingest_scip_calls(&conn, &fn_registry, idx_path.as_path(), cli.repo_id.as_str()) {
            Ok((calls_written, defs_in_scip)) => {
                eprintln!(
                    "scip: {} Definition anchors in Rust documents; {} CALLS edges created",
                    defs_in_scip, calls_written
                );
            }
            Err(err) => eprintln!("ERR scip ingest: {err}"),
        }
    }

    Ok(())
}

fn posix_path_display(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Index one file: delete derivatives, pending upserts → embed → Chunk rows → completion.
///
/// Embedding failures roll back semantics at the Chunk layer only indirectly: we detach-delete
/// per-chunk ids before recreate; callers may choose to rerun the whole file.
fn index_one_file(
    conn: &lbug::Connection<'_>,
    http: &reqwest::blocking::Client,
    jopts: &jina::JinaEmbedOpts<'_>,
    repo_id: &str,
    repo_root_path: &Path,
    repo_root_disp: &str,
    rel_posix: &str,
    file_stem: String,
    lang: parse::LangPack,
    embed_dims: usize,
    batch_units: usize,
    fn_registry: &mut FunctionAnchorRegistry,
) -> Result<(), String> {
    let abs = join_rel_paths(repo_root_path, rel_posix);
    let bytes = std::fs::read(&abs).map_err(|e| format!("read {}: {e}", abs.display()))?;
    let md5_hex = ids::md5_hex(&bytes);
    let Ok(source) = String::from_utf8(bytes) else {
        return Ok(()); // skip binary-ish file quietly
    };

    let fid = ids::file_id(repo_id, rel_posix);
    let doc_pk = ids::document_id(repo_id, rel_posix);

    delete_file_derivatives(conn, repo_id, rel_posix)?;

    upsert_code_document_pending(
        conn,
        &doc_pk,
        repo_id,
        rel_posix,
        &file_stem,
        &md5_hex,
        repo_root_disp,
    )?;

    upsert_file_pending(
        conn,
        &fid,
        repo_id,
        rel_posix,
        &file_stem,
        &md5_hex,
        repo_root_disp,
    )?;

    let units = extract_units(rel_posix, lang, &source);
    if batch_units == 0 {
        return Err("--batch-units must be positive".into());
    }

    for slice in units.chunks(batch_units) {
        let mut passages: Vec<String> = Vec::with_capacity(slice.len());
        let mut trims: Vec<String> = Vec::with_capacity(slice.len());
        for u in slice {
            let t = trim_for_embedding(u.source_text.as_str());
            passages.push(t.clone());
            trims.push(t);
        }

        let vectors = jina::embed_passages_blocking(http, &passages, jopts)?;
        for ((unit, trimmed_for_emb), embedding) in slice.iter().zip(trims.into_iter()).zip(vectors.into_iter()) {
            if embedding.len() != embed_dims {
                return Err(format!(
                    "embedding width {} differs from configured {} for {}",
                    embedding.len(),
                    embed_dims,
                    unit.signature
                ));
            }

            let target_pk = if unit.target_table == "Class" {
                let cid = ids::class_id(repo_id, unit.signature.as_str());
                upsert_class_node(
                    conn,
                    &cid,
                    repo_id,
                    rel_posix,
                    &short_tail_name(&unit.qualified_name),
                    unit.qualified_name.as_str(),
                    unit.signature.as_str(),
                    trimmed_for_emb.as_str(),
                )?;
                create_rel_defines(conn, fid.as_str(), "Class", &cid)?;
                cid
            } else {
                let fun = ids::function_id(repo_id, unit.signature.as_str());
                upsert_function_node(
                    conn,
                    &fun,
                    repo_id,
                    rel_posix,
                    &short_tail_name(&unit.qualified_name),
                    unit.qualified_name.as_str(),
                    unit.signature.as_str(),
                    trimmed_for_emb.as_str(),
                    unit.name_line,
                )?;
                fn_registry.record_function_anchor(rel_posix, unit.name_line, fun.clone());
                create_rel_defines(conn, fid.as_str(), "Function", &fun)?;
                fun
            };

            let chunk_props = serde_json::json!({
                "embedding_model": jopts.model,
                "embedding_task": jopts.task,
                "target_kind": unit.target_kind,
                "signature": unit.signature,
                "qualified_name": unit.qualified_name,
                "name_line": unit.name_line,
                "storage_mode": "native",
                "dimensions": embed_dims,
            })
            .to_string();

            let cid = ids::chunk_id(
                repo_id,
                unit.target_kind,
                unit.signature.as_str(),
                trimmed_for_emb.as_str(),
            );

            insert_chunk_dense(
                conn,
                &cid,
                repo_id,
                rel_posix,
                short_tail_name(&unit.qualified_name).as_str(),
                embedding.as_slice(),
                chunk_props.as_str(),
                trimmed_for_emb.as_str(),
            )?;

            create_rel_describes(conn, cid.as_str(), unit.target_kind, target_pk.as_str())?;
        }
    }

    mark_file_and_document_complete(
        conn,
        doc_pk.as_str(),
        fid.as_str(),
        repo_id,
        rel_posix,
        &file_stem,
        &md5_hex,
        repo_root_disp,
    )
}

/// Join POSIX `repo_root/relative` segments regardless of OS — repo roots are canonically resolved first.
fn join_rel_paths(repo_root_path: &Path, rel_posix: &str) -> PathBuf {
    let mut p = repo_root_path.to_path_buf();
    for seg in rel_posix.split('/').filter(|s| !s.is_empty()) {
        p.push(seg);
    }
    p
}

/// Last path segment after `::` suitable for Chunk `name` / node `name` short label.
fn short_tail_name(q: &str) -> String {
    q.rsplit("::").next().unwrap_or(q).to_string()
}
