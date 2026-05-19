# Agent guide — `am-workspace` (`am-rust-full`)

Practical instructions for coding agents working in this crate. Prefer this file over guessing paths or conflating this server with hosted Agentic Memory backends.

---

## 1. What this crate is (and is not)

**Is:**

- **`workspace-server`**: Axum app merging API routes (`src/server/routes/`) and static SPA files from **`DIST_PATH`** (`src/server/static_files.rs`).
- **Ladybug integration**: Opens a **filesystem** `.lbug` via `store::ladybug` (`src/server/store/ladybug.rs`). Used by graph handlers in `src/server/routes/graph.rs`.
- **Optional native `lbug` crate**: Feature **`ladybug`** (default). Without it, `lbug_shim` provides empty Ladybug rows; **`workspace-server`** still needs feature **`server`** (Axum/Tokio).

**Is not:**

- A client for **`https://backend.agentmemorylabs.com`** or **`https://rt-*.agentmemorylabs.app/mcp-full/`**. Those are Python MCP / managed flows; this Rust server does not substitute them.
- A replacement for **`agent-memory` CLI** configuration in the parent repo. CLI + Codex MCP live outside this crate.

When the user asks for “deployed backend” wiring, clarify: **hosted MCP/CLI** vs **local `.lbug` + workspace-server** are different surfaces unless someone adds an HTTP client later.

---

## 2. Repo layout (high-signal paths)

| Path | Role |
|------|------|
| `src/server/main.rs` | Server entry: config, `open_ladybug_db`, router merge |
| `src/server/config.rs` | `PORT`, `WORKSPACE_*`, `DIST_PATH` from env |
| `src/server/store/ladybug.rs` | DB path discovery (`LADYBUG_DB_PATH`, store, `~/.agentic-memory`), open, Cypher helpers |
| `src/server/routes/graph.rs` | Graph REST handlers |
| `src/lib.rs` | Library crate `am_workspace`; `egui` feature gates `app`, `canvas`, etc.; WASM `#[wasm_bindgen(start)]` |
| `src/app_main.rs` | Native `workspace-app` (`eframe`) entry |
| `index.html` | Trunk root for WASM build |
| `scripts/build_workspace_server.ps1` | **Windows canonical** native server build + DLL copy |
| `.env.example` | Documented env vars (no secrets committed) |

**Important:** The library intentionally does **not** re-export a duplicate `server` module tree. Server code lives only under `src/server/` as part of the **`workspace-server`** binary sources.

---

## 3. Build commands

**Browser bundle (served by workspace-server):**

```powershell
cd D:\code\agentic-memory\am-rust-full
trunk build --release
# Same as: trunk build --release --no-default-features --features egui (see Trunk.toml)
```

**Server with native Ladybug (Windows):**

```powershell
pwsh -ExecutionPolicy Bypass -File .\scripts\build_workspace_server.ps1 -Profile release
# Optional if default target dir is full:
# ... -CargoTargetDir D:\cargo-target-am-workspace
```

**Shim-only (CI / no DLL):**

```powershell
cargo build --bin workspace-server --no-default-features --features server
```

**Desktop egui:**

```powershell
cargo run --bin workspace-app --no-default-features --features egui
```

---

## 4. Run + verify

```powershell
$env:LADYBUG_DB_PATH = "D:\absolute\path\to\file.lbug"
.\target\release\workspace-server.exe
```

Smoke HTTP shape (does not validate UI):

```powershell
pwsh -File .\scripts\smoke_workspace_graph.ps1 -ExePath .\target\release\workspace-server.exe
```

Non-empty Ladybug graph data requires **real `.lbug`** + **`ladybug`** feature build + successful native open (check logs for `LadybugDB graph integration active`).

---

## 5. Frequent pitfalls

1. **Windows link / DLL**: Static MSVC artifacts for `lbug` are easy to get wrong. Use `build_workspace_server.ps1` or follow **`lbug-crate-windows.md`** (`LBUG_SHARED`, merged headers, `lbug_shared.dll` beside exe).
2. **Disk space**: Default `target/` can grow large; use `--target-dir` or `-CargoTargetDir`.
3. **`DIST_PATH`**: Relative to **process working directory**. Prefer absolute path if the user starts the exe from elsewhere.
4. **Axum static files**: Root route uses `ServeDir` + **`fallback(ServeFile::index)`** — do not reintroduce nesting `ServeDir` at router root in ways Axum 0.8 rejects.
5. **WAL / corruption**: Treat live `.lbug` carefully; backup before experiments (see parent repo hosting docs).
6. **Trunk / WASM**: Do not enable the **`server`** feature on `wasm32-unknown-unknown` — **`tokio`** pulls **`mio`**, which does not build for WASM. Use **`trunk build --no-default-features --features egui`**.

---

- Graph behavior and Ladybug queries: **`store/ladybug.rs`** + **`routes/graph.rs`**.
- New env knobs: **`config.rs`**, **`.env.example`**, and **`README.md`** / this file.
- Do not expand scope into parent **`agentic-memory`** Python MCP unless the task explicitly requires it.

---

## 6.1 TypeScript reference features — port status

Reference dashboard: `agentic-memory-obsidian-clone/packages/am-dashboard`. Gap decisions: `D:\code\agentic-memory\.planning\execution-am-rust-full\PORT_GAP_PLAN.md`.

| Feature | Status |
|---------|--------|
| **Excalidraw draw mode** | **adapt** — hybrid: `engine: excalidraw` on board + Draw view; static `dist/excalidraw-bridge.html` copied by Trunk from `static/excalidraw-bridge.html`. |
| **Mermaid → drawing** | **defer** — needs Excalidraw/DOM pipeline; not in egui core. |
| **Agent chat / edit proposals** | **adapt** — local fallback + optional `AM_AGENT_PROVIDER_URL` stub; not hosted MCP. |
| **OpenClaw shell metrics** | **out-of-crate** — use neutral **Diagnostics** panel (Ladybug up, attempts). |
| **Cypher shell** | **port** — read-only `/api/workspace/query/execute` + Query UI. |
| **Rich markdown toolbar** | **port** — egui formatting toolbar in editor. |
| **Repo / project scope filters** | **port** — sidebar scope + graph explore query params. |
| **Board ingest** | **port** — server routes + canvas **Ingest board** + client API. |

Core Rust scope: notes, boards, canvas, graph explorer, diagnostics, query shell, agent panel (local), Ladybug graph APIs with observability attempts.

---

## 7. Cross-links

- End-user overview: [`README.md`](README.md)
- Windows Ladybug headers/libs: [`lbug-crate-windows.md`](lbug-crate-windows.md)
- Historical phase notes: [`PHASE_4_COMPLETE.md`](PHASE_4_COMPLETE.md)
- Monorepo agent rules: `D:\code\agentic-memory\AGENTS.md` (product-wide; Ladybug Sprite vs VM assumptions live there)
