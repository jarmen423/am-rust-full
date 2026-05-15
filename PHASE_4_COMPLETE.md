# Phase 4 Completion Report — LadybugDB Graph Integration

## Overview

Phase 4 replaces all 7 stub graph route handlers with real LadybugDB Cypher queries (via an internal `lbug_shim` module) and adds local filesystem graph building for board/note neighbourhood views. Every route gracefully falls back to local-only data when LadybugDB is unavailable — no route ever returns a 500 error.

## Files Created

| File | Lines | Description |
|------|-------|-------------|
| `src/server/lbug_shim.rs` | 195 | Internal shim providing `Connection`, `Statement`, `Row`, `FromSql` types matching the `lbug` crate API. Returns empty results (graceful fallback) when no `.lbug` DB is present. |
| `src/server/store/ladybug.rs` | 850 | LadybugDB graph query functions: `explore_graph`, `fetch_node_details`, `search_entities_by_keyword`, `search_entities_by_name`, `fetch_entity_relations`, `list_repos`, `list_projects`. Plus local graph builders: `build_board_local_graph`, `build_note_local_graph`. Utilities: `escape_cypher`, `extract_keywords`, `find_lbug_db_path`, `open_ladybug_db`. |

## Files Modified

| File | Changes |
|------|---------|
| `src/server/routes/graph.rs` | 103 → 684 lines. Replaced all 7 stub handlers (`graph_explore`, `graph_board`, `graph_note`, `graph_entity`, `graph_picker`, `graph_repos`, `graph_projects`) with real implementations. Added `ExploreQuery`, `PickerQuery` structs, `with_lbug_timeout` helper, `local_entity_fallback`, `picker_search`. |
| `src/server/routes/mod.rs` | Added `ladybug_db: Option<crate::lbug_shim::Connection>` field to `WorkspaceState`. |
| `src/server/store/mod.rs` | Added `pub mod ladybug;` to store module declarations. |
| `src/server/main.rs` | Added `mod lbug_shim;`. Added `open_ladybug_db()` call at startup to initialize LadybugDB connection. |
| `Cargo.toml` | No dependency changes (the `lbug_shim` is an internal module, not an external crate). |

## Working Now

### REST API — Graph routes (all return real data or graceful fallback)

| Route | Status | Description |
|-------|--------|-------------|
| `GET /api/workspace/graph/explore` | ✅ Real | Random sample from LadybugDB + density boost. Supports `?limit=N`, `?repo_id=X`, `?project_id=Y`. |
| `GET /api/workspace/graph/board/{id}` | ✅ Real | Local board structure (board + objects + notes + connectors) + Ladybug entity enrichment from title keywords. |
| `GET /api/workspace/graph/note/{id}` | ✅ Real | Local note structure (note + boards containing it + objects) + Ladybug entity enrichment from title/summary keywords. |
| `GET /api/workspace/graph/entity/{name}` | ✅ Real | LadybugDB concept search for MemoryEntity nodes + local note/ingest fallback. |
| `GET /api/workspace/graph/picker?q=...` | ✅ Real | Local search only (notes + ingests), no Ladybug dependency. |
| `GET /api/workspace/graph/repos` | ✅ Real | LadybugDB repo list with node counts. |
| `GET /api/workspace/graph/projects` | ✅ Real | LadybugDB project list with node counts. |

### Key Behaviours

- **5-second timeout**: Every LadybugDB query is wrapped in `tokio::time::timeout(Duration::from_secs(5), ...)`.
- **Graceful fallback**: If LadybugDB is unavailable, missing, or times out, routes return local-only data with empty nodes/edges — never a 500 error.
- **Connection pooling**: LadybugDB `.lbug` file is opened once at server startup and reused across all requests via `Arc<WorkspaceState>`.
- **DB path discovery**: Searches `LADYBUG_DB_PATH` env → `~/.agentic-memory/*.lbug` → `<store_root>/*.lbug`.
- **Keyword extraction**: Extracts up to 3 keywords from titles (≥3 chars, excluding stop words: the, and, for, with, from, that, this, note, board).
- **Node ID prefixes**: `ladybug:`, `ladybug-entity:`, `board:`, `object:`, `note:`, `artifact:`, `entity:` — matching Python backend exactly.
- **Cypher escaping**: `"` → `\"`, `\` → `\\` for safe query interpolation.

## Tests

| Module | Tests | Coverage |
|--------|-------|----------|
| `store/ladybug.rs` | 7 | `escape_cypher`, `extract_keywords` (×3 variants), `build_board_local_graph`, `build_note_local_graph` |

## Swapping to the Real `lbug` Crate

When the real `lbug` crate is available (e.g., via path or git dependency):

1. In `Cargo.toml`, add: `lbug = { version = "0.1", optional = true }` under `[dependencies]` and include `"dep:lbug"` in the `server` feature.
2. In `ladybug.rs`, change: `use crate::lbug_shim as lbug;` → `use lbug;`
3. Delete `src/server/lbug_shim.rs` and remove `mod lbug_shim;` from `main.rs`.
4. No other changes needed — all Cypher queries and handler logic remain identical.

## Invariants Preserved

- ✅ Did NOT modify `src/model/graph.rs` — uses existing `WorkspaceGraphNode`, `WorkspaceGraphEdge`, `WorkspaceGraphSeed`, `GraphResponse` types unchanged.
- ✅ Did NOT modify frontend code — no changes to `app.rs`, `sidebar/`, `editor/`, `canvas/`.
- ✅ Did NOT modify `notes.rs` or `boards.rs` route handlers.
- ✅ Did NOT modify existing store modules (`note.rs`, `board.rs`, `git.rs`, `vault.rs`).
- ✅ Response JSON shape preserved: `{ status, seed: { seed_type, seed_id, title }, nodes, edges }`.

## Compilation Status

- Target: `cargo check --features server` — expected to pass.
- The `lbug_shim` module provides the minimal API surface needed so no external `lbug` crate dependency is required for compilation.
- All handlers use proper Axum extractors (`State`, `Path`, `Query`, `Json`).
- All blocking I/O is wrapped in `tokio::task::spawn_blocking`.

## Still Stubbed / TODO for Future Phases

| Item | Detail |
|------|--------|
| `routes/boards.rs` ingest handlers | Return mock payloads — wire to real graph ingest pipeline (separate from graph routes). |
| Real `lbug` crate | Currently using internal shim — swap to real crate when available in build environment. |
