# Agent notes — `repo_jina_lb`

## What this is

Rust pipeline: walk a repo (`.gitignore`-aware), tree-sitter structural units → **Jina** `code.passage` embeddings → **Ladybug** native graph (`CodeDocument`, `File`, `Function`, `Class`, `Chunk` with `FLOAT[N]`, `DEFINES`, `DESCRIBES`).

Schema intent mirrors Agentic Memory Python **`CODE_SCHEMA`** (`agentic_memory/ladybug/schema.py`). This crate duplicates DDL in `schema_ddl.rs`; keep aligned when upstream schema changes.

## Where to edit

- **In-tree module**: `src/repo_jina_lb/*.rs`, wired from [`src/lib.rs`](../../lib.rs) as `repo_jina_lb` behind feature **`jina-ladybug-index`**.
- **Standalone crate**: [`standalone-pack/`](./standalone-pack/) — copy-out-friendly layout (`Cargo.toml`, `src/lib.rs`, …).

When changing ingest logic, update **both** unless you intentionally fork—see [`README.md`](./README.md) sync note.

## Build surfaces

| Context | Command sketch |
|---------|----------------|
| Workspace binary | `cargo run -p am-workspace --features jina-ladybug-index --bin jina-ladybug-repo-index -- …` |
| Standalone pack | **`May be incomplete`;** after copying from **`standalone-pack/`**, ensure all `*.rs` from parent **`repo_jina_lb`** are present (`lib.rs` from `mod.rs`), then run `cargo run --release -- …`. |


## Secrets & env

- **`JINA_API_KEY`** — bearer token for `https://api.jina.ai/v1/embeddings`.
- Ladybug link vars: **`LBUG_*`** (platform-specific; Windows shared DLL path is common).

Do not commit `.env`, API keys, or `.lbug` databases.

## Failure modes / ops

- **Link errors on Windows**: usually wrong Ladybug artifact (prefer shared DLL flow — `lbug-crate-windows.md` at workspace root).
- **Embedding width mismatch**: `--dimensions` must match stored DDL for `Chunk.embedding`.
- **429 / HTTP errors from Jina**: reduce `--batch-units`, retry; failures print `ERR <path>: …` per file.

## Incremental ingest

Same path lines **`ok`** (full Jina + Chunk path) vs **`skip`** when DB row is **`complete`**, **`source_hash`** matches disk MD5, and **`properties_json.jina_fingerprint`** matches **`model|task|dimensions`**. **`--force-reindex`** forces the heavy path regardless. Older DBs without **`jina_fingerprint`** ingest fully once to populate it. SCIP **`CALLS`** alignment still parses every skipped file.

## Tests

There is no dedicated integration test harness in `standalone-pack` yet; validate with a scratch `.lbug` and small repo path after schema init.
