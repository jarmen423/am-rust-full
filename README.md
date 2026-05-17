# am-workspace (`am-rust-full`)

Rust workspace UI and HTTP server for **Agentic Memory Workspace**: egui front end (native WASM or desktop), local workspace store/vault, and **LadybugDB** graph APIs backed by a filesystem `.lbug` file.

This crate lives under the **agentic-memory** monorepo as `am-rust-full`. Managed hosted flows (`agent-memory` CLI, OpenClaw, `backend.agentmemorylabs.com`) are separate; this binary talks to **local paths**, not the Sprite MCP URL.

---

## What you get

| Component | Binary / artifact | Role |
|-----------|-----------------|------|
| **HTTP + API + static UI** | `workspace-server` | Axum server: `/api/workspace/*`, serves `DIST_PATH` (default `dist/`) |
| **Desktop egui** | `workspace-app` | Same UI via `eframe` (no browser) |
| **Optional indexer** | `jina-ladybug-repo-index` | Jina/scip → Ladybug code graph (`--features jina-ladybug-index`) |

Graph routes (`/api/workspace/graph/*`) use **native Ladybug** when built with the **`ladybug`** feature (on by default) and a reachable `.lbug`. Disable **`ladybug`** only (keep **`server`**) to use the stub shim — empty Ladybug rows, still compiles **`workspace-server`**.

**Trunk / WASM:** enable **`egui`** and disable **defaults** (`trunk build --no-default-features --features egui`). The **`server`** stack pulls **`tokio` → `mio`**, which does **not** compile for `wasm32-unknown-unknown`.

---

## Prerequisites

- **Rust** (stable), `rustup`
- **Wasm browser UI:** `rustup target add wasm32-unknown-unknown` and [Trunk](https://trunkrs.dev/) (`cargo install trunk`)
- **Windows + native Ladybug:** see [`lbug-crate-windows.md`](lbug-crate-windows.md) and prefer [`scripts/build_workspace_server.ps1`](scripts/build_workspace_server.ps1) so `lbug_shared.dll` sits next to `workspace-server.exe`

---

## Quick start (browser)

From this directory (`am-rust-full`):

```powershell
# 1) Frontend bundle → dist/ (Trunk.toml turns off crate defaults for WASM)
trunk build --release
# Equivalent explicit flags: trunk build --release --no-default-features --features egui

# 2) Server (Windows: prepares shared LBUG + copies DLL beside exe)
pwsh -ExecutionPolicy Bypass -File .\scripts\build_workspace_server.ps1 -Profile release

# 3) Point at your DB and run (absolute path recommended)
$env:LADYBUG_DB_PATH = "D:\path\to\your.db.lbug"
$env:PORT = "3031"
.\target\release\workspace-server.exe
```

Open **http://127.0.0.1:3031**. Run the server from this repo root **or** set `DIST_PATH` to an absolute path to your built `dist` folder.

### Trunk: `mio` / “wasm target is unsupported”

Cargo **unifies features for the whole package**. Default features include **`server`** → **`tokio`** → **`mio`**, which **does not build** for `wasm32-unknown-unknown`.

Use **`Trunk.toml`** in this repo (it sets `no_default_features = true` and `features = ["egui"]`), so **`trunk build --release`** is enough; or pass explicitly:

`trunk build --release --no-default-features --features egui`

---

## Quick start (native desktop app)

```powershell
cargo run --bin workspace-app --no-default-features --features egui
```

Uses the same egui code paths as WASM; does not start `workspace-server`.

---

## Environment variables

| Variable | Default / notes |
|----------|------------------|
| `PORT` | `3031` |
| `DIST_PATH` | `dist` — Trunk output directory for static files |
| `WORKSPACE_STORE_PATH` | `$HOME/.agentic-memory/workspace-store` (Unix) or override |
| `WORKSPACE_VAULT_PATH` | `$HOME/.agentic-memory/workspace-vaults` or override |
| `LADYBUG_DB_PATH` | Optional explicit `.lbug` file; else discovery under store / `~/.agentic-memory` |
| `RUST_LOG` | e.g. `info,tower_http=debug` |

Copy [`.env.example`](.env.example) to `.env` locally if you use a loader; `.env` is gitignored.

On Windows, **`LBUG_SHARED`**, **`LBUG_LIBRARY_DIR`**, and **`LBUG_INCLUDE_DIR`** are set by `build_workspace_server.ps1` during link (see script and `lbug-crate-windows.md`).

---

## Cargo features

| Feature | Meaning |
|---------|---------|
| `server` (**default**) | Axum, Tokio, Git2, tracing — required for **`workspace-server`** (not for WASM lib) |
| `ladybug` (**default**) | Link optional `lbug` crate for real graph queries |
| `ladybug` off, `server` on | `lbug_shim` only — compile **`workspace-server`** without native Ladybug DLL |
| `egui` | Frontend (Trunk WASM + `workspace-app`) |
| `jina-ladybug-index` | `jina-ladybug-repo-index` binary |

Example **`workspace-server`** without native Ladybug (still needs **`server`**):

```powershell
cargo build --bin workspace-server --no-default-features --features server
```

---

## Scripts

| Script | Purpose |
|--------|---------|
| [`scripts/build_workspace_server.ps1`](scripts/build_workspace_server.ps1) | Windows: fetch/prepare shared Ladybug artifact, `cargo build --bin workspace-server`, copy `lbug_shared.dll` |
| [`scripts/smoke_workspace_graph.ps1`](scripts/smoke_workspace_graph.ps1) | Start server briefly; hit `graph/explore` and `graph/repos` |
| [`scripts/smoke_managed_backend.ps1`](scripts/smoke_managed_backend.ps1) | Curl `backend.agentmemorylabs.com` health + onboarding (no credentials) |

---

## Notes on `.lbug` files

- The server opens the database from disk (**read-oriented** integration). It does **not** call hosted MCP or `rt-*.agentmemorylabs.app` over HTTP.
- Sync or copy a `.lbug` from your Ladybug runtime per your ops docs. If open fails with WAL corruption, back up the file and WAL and repair/quarantine before retrying — do not treat production DBs as disposable scratch.

---

## Further reading

- [`PHASE_4_COMPLETE.md`](PHASE_4_COMPLETE.md) — graph routes and Ladybug behavior (some notes pre-date the optional `lbug` crate wiring)
- [`lbug-crate-windows.md`](lbug-crate-windows.md) — MSVC include merge and `LBUG_*` variables
- Parent repo: `D:\code\agentic-memory` — `agent-memory` CLI, MCP, and full-stack docs
