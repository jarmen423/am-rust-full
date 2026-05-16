# Jina embeddings → Ladybug (`jina-ladybug-repo-index`)

This tool walks a repo, chunks code with tree-sitter, calls **Jina** (`code.passage`), and writes **`CODE_SCHEMA`** rows into a Ladybug **`.lbug`** file.

---

## 1. Build (already done)

```powershell
Set-Location "D:\code\agentic-memory\am-rust-full"
cargo build -p am-workspace --bin jina-ladybug-repo-index --features jina-ladybug-index
```

On Windows you may still need **`LBUG_*`** env vars **only while running `cargo build`** (shared DLL + merged headers). That has nothing to do with the **run** steps below unless you rebuild.

---

## 2. Run the indexer (after build)

The executable is:

```text
D:\code\agentic-memory\am-rust-full\target\debug\jina-ladybug-repo-index.exe
```

### Jina API key

You must give Jina a bearer token **every time you index**, either:

- **Environment variable** (works from any terminal):

  ```powershell
  $env:JINA_API_KEY = "paste-your-key-here"
  ```

- **Or** pass **`--jina-api-key`** on the command line (same effect).

(Earlier wording about “same PowerShell session” only meant: if you **only** set `$env:JINA_API_KEY` in one window and never saved it globally, **that variable disappears when you close that window**—set it again next time, or use `--jina-api-key`.)

### Ladybug DLL at runtime

Windows must find **`lbug_shared.dll`** when you run the `.exe` (same rule as any DLL): either put it **next to** `jina-ladybug-repo-index.exe`, or add its folder to your **`PATH`**.

---

## 3. Example command

```powershell
Set-Location "D:\code\agentic-memory\am-rust-full"
$env:JINA_API_KEY = "paste-your-key-here"

.\target\debug\jina-ladybug-repo-index.exe `
  --repo "D:\some\repo\to\index" `
  --db "D:\path\to\my-code-graph.lbug" `
  --repo-id "choose-a-stable-id-for-this-repo" `
  --init-schema
```

| Flag | Meaning |
|------|--------|
| **`--repo`** | Root folder to scan (respects `.gitignore`). |
| **`--db`** | Path to the **`.lbug`** database file (created if missing when schema runs). |
| **`--repo-id`** | Stored on every row (pick something stable for that codebase). |
| **`--init-schema`** | **First time only** for this DB: create Ladybug tables + vector bits. Omit next time unless you deleted the DB. |
| **`--scip`** | *(Optional)* Path to **`index.scip`**. If omitted, search `index.scip` / `target/index.scip` / `.scip/index.scip` under `--repo`. Requires **rust-analyzer**-style SCIP for Rust `CALLS`. |

Embedding width defaults to **`2048`** (Jina v4 dense max). Use **`--dimensions`** only if you intentionally changed how you created **`Chunk.embedding`** in that DB.

---

## 4. Optional: `CALLS` edges (rust-analyzer SCIP)

`CODE_SCHEMA` includes **`CALLS` (`Function → Function`)**, but tree-sitter + Jina **do not** populate call edges alone. After the embedding walk finishes, pass an **`index.scip`** produced by **rust-analyzer**: the indexer writes **`CALLS`** aligned by **definition line + path**.

### Produce `index.scip`

From the **same directory** as `--repo`:

```powershell
Set-Location "D:\path\to\your\repo"
rust-analyzer scip .
```

This usually creates **`index.scip`** in the repo root. Override with **`--scip`** if you store it elsewhere.

If **`--scip`** is omitted, search order under `--repo` is: `index.scip`, then `target/index.scip`, then `.scip/index.scip`.

### Rust-only v1

- Only SCIP documents with `language` starting with **`rust`** are used.
- **`Relationship { is_reference: true }`** from callable symbols (function, method, …) is turned into **`CALLS`** when both endpoints resolve to **`Function`** rows from this run (via line anchors). Unresolved pairs are skipped.
- “Reference” is broader than “call”; expect some non-call edges until heuristics tighten.

---

## 5. Help / options

```powershell
.\target\debug\jina-ladybug-repo-index.exe --help
```

---

## 6. Optional: nicer builds if `target` acts weird

If `cargo` warns about **`incremental`** / “access denied”, you can redirect build output:

```powershell
$env:CARGO_TARGET_DIR = "$env:TEMP\am-rust-full-target"
cargo build -p am-workspace --bin jina-ladybug-repo-index --features jina-ladybug-index
```

Then run the `.exe` from **`am-rust-full\target\debug\`** only if you did **not** set `CARGO_TARGET_DIR`; if you did, the exe lives under **`%TEMP%\am-rust-full-target\debug\`**.
