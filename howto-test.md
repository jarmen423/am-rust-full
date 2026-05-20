Build once
```shell
# WASM UI → dist/ (includes excalidraw-bridge.html)
$env:NO_COLOR = $null
trunk build --release

# Server (Ladybug DLL on Windows)
pwsh -ExecutionPolicy Bypass -File .\scripts\build_workspace_server.ps1 -Profile release
```
Run the server 
```shell
cd D:\code\agentic-memory\am-rust-full

# Optional but recommended for graph + query
$env:LADYBUG_DB_PATH = "D:\path\to\your.db.lbug"   # e.g. from .env.example

$env:PORT = "3031"
$env:RUST_LOG = "info"
.\target\release\workspace-server.exe
```
Keep process running from repo root so that `DIST_PATH=dist` resolves.
Open: `http://127.0.0.1:3031`

3. Click through the UI (top tabs)

**Important:** The HTML `<canvas id="canvas">` is the egui WASM surface for the whole app — not a separate graph or infinite canvas DOM layer.

| Tab | What you should see |
|-----|---------------------|
| **Canvas** | Click a **board** under **Boards** — floating **Tools** bar at bottom. **Note** (▤) + click canvas = new note card. Pan: ✥ or Space+drag. **Select** (⬚) to move/resize cards. |
| **Graph** | Nodes only if **LadybugDB** has indexed data (`LADYBUG_DB_PATH`). Sidebar notes ≠ graph nodes. **Diag** → Ladybug: up, then **Refresh**. |
| **Draw** | Excalidraw JSON + link to bridge page — not the infinite note canvas. |

4. Excalidraw bridge (separate page)
From draw tab click **open excalidraw bridge** or go directly to: 
`http://127.0.0.1:3031/excalidraw-bridge.html`

You should see:
- toolbar: export/import scene json
- iframe to excalidraw.com (manual editing; not auto-syncd to board)
- text that mermaid conversion is not implemented

## Canvas note cards (resize + markdown preview)

After changing canvas code, rebuild WASM and hard-refresh the browser:

```shell
trunk build --release
# Ctrl+F5 at http://127.0.0.1:3031
```

1. **Resize:** Canvas tab → **Select** (⬚) → hover a note card (cursor changes on edges/corners; no visible grab dots) → drag any **edge** or **corner** to resize. Drag the **interior** to move. While resizing, status shows size e.g. `340 × 220`.
2. **Connect tool:** Switch to **Connect** (↗) → hover a card → teal mid-edge dots appear for linking (not shown in Select mode).
3. **Markdown preview:** Edit a note in **Notes** (headings, `**bold**`, `- bullets`, fenced ` ```python ` blocks) → **Save** → **Canvas**. Card title/preview should update. Fenced code renders as a dark monospace block. Taller cards show more lines (resize with Select + edge/corner drag).
4. **Note 17 smoke test:** Save note with a code block at the bottom → Canvas → drag bottom edge or bottom-right corner until the gray code block is visible.

## Desktop App (limited)
`cargo run --bin workspace app --no-default-features --features egui`
- this shows the same egui layout but does not start `workspace-server`.
    - api calls use rel paths so notes/graph/query/agent usually wont work unless you've wired a base url elsewhere. For testing ports, use the browser flow above. 