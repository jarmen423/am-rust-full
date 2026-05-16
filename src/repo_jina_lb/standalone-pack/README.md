# `jina-ladybug-repo-index`

Embed repository source with **[Jina](https://jina.ai/) `jina-embeddings-v4`** (`task: code.passage`) and persist results into a native **[Ladybug](https://crates.io/crates/lbug)** `.lbug` database using Agentic Memory’s **CODE_SCHEMA** shape (`CodeDocument`, `File`, `Function`, `Class`, `Chunk` with dense `FLOAT[N]` embeddings, plus `DEFINES` / `DESCRIBES`).

## Features

- `.gitignore`-aware repo walk (`ignore` crate).
- Tree-sitter structural units for Rust, Python, JS/TS/TSX (`parse`).
- Batched HTTPS calls to Jina embeddings API (`jina`).
- Prepared statements + Cypher for graph writes (`ladybug_writes`), **`DETACH DELETE` before `CREATE`** per deterministic chunk id.
- Optional **`CALLS`** edges (**Rust v1**) from **`index.scip`** (`scip_calls`; aligns anchors via `calls_registry`, populated during ingest).
- Incremental **`ok` / `skip`** ingest when **`jina_fingerprint`** + **`source_hash`** match (see **`jina-embeddings.md`** beside **`am-workspace`** / parent **`repo_jina_lb`** docs).

## Quickstart

See parent **[`install.md`](../install.md)** (monorepo) for extract-from-workspace notes, Windows Ladybug DLL setup, and example `cargo run` lines.

Minimal run:

```bash
export JINA_API_KEY=...
cargo run --release -- \
  --repo . \
  --db ./code.lbug \
  --repo-id my-repo \
  --init-schema \
  --dimensions 2048
```

## Crate layout

| Path | Role |
|------|------|
| `src/lib.rs` | CLI (`clap`), orchestration, per-file ingest |
| `src/main.rs` | Thin binary wrapper |
| `src/jina.rs` | Jina REST client |
| `src/parse.rs` | Tree-sitter chunking |
| `src/ladybug_writes.rs` | Native Ladybug writes (+ **`CALLS`** helpers) |
| `src/scip_calls.rs` | rust-analyzer SCIP → `CALLS` |
| `src/calls_registry.rs` | Path + line anchors linking TS ingest ↔ SCIP |
| `src/schema_ddl.rs` | DDL + vector index statements |
| `src/ids.rs` | Stable pk conventions |

## Origin

Extracted from the **`am-workspace`** **`repo_jina_lb`** module (`src/repo_jina_lb/`). This **`standalone-pack/`** checkout may omit some mirrored **`src/*.rs`** files; **`cargo build` here may fail until you copy sources from the parent module (typically use **`mod.rs`** as **`src/lib.rs`**) plus the sibling `*.rs` files. See parent **[`README.md`](../README.md)** and **[`install.md`](../install.md)**.

When contributing upstream, reconcile changes with the in-tree **`repo_jina_lb`** sources.

## License

Specify to match your organization when publishing (`Cargo.toml` lists `MIT OR Apache-2.0` as a placeholder).
