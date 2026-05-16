# Install & extract (`jina-ladybug-repo-index`)

## Extract into its own repo

1. Copy the directory **`standalone-pack/`** (under `src/repo_jina_lb/`) to a new location, e.g.:

   ```powershell
   Copy-Item -Recurse "D:\code\agentic-memory\am-rust-full\src\repo_jina_lb\standalone-pack" "D:\code\jina-ladybug-repo-index"
   ```

2. Open that folder — it must contain `Cargo.toml`, `src/`, `README.md`, `install.md`, `.gitignore`.

3. Build:

   ```powershell
   cd D:\code\jina-ladybug-repo-index
   cargo build --release
   ```

4. Initialize git:

   ```powershell
   git init
   git add .
   git commit -m "Initial import: jina-ladybug-repo-index"
   ```

## Prerequisites

- **Rust** stable (`rustup`), recent Cargo.
- **[Jina AI](https://jina.ai/) API key** — set `JINA_API_KEY` or pass `--jina-api-key`.
- **Ladybug (`lbug`) native library** — the `lbug` crate expects linkable Ladybug binaries for your platform.

### Windows + Ladybug (`lbug`)

Static MSVC artifacts may fail link with unresolved symbols. Prefer the **official shared DLL** workflow documented next to this repo:

- See **`lbug-crate-windows.md`** at the **`am-rust-full`** workspace root (merged headers + `LBUG_SHARED=1` + `lbug_shared.dll` beside the binary).

Set env vars before `cargo build`, for example:

- `LBUG_SHARED=1`
- `LBUG_LIBRARY_DIR`, `LBUG_INCLUDE_DIR` pointing at prepared artifact dirs  
  (or use the upstream project’s `build_embedded.ps1` pattern).

Linux/macOS: follow `lbug` crate / upstream Agentic Memory worker docs (`build_embedded.sh`, shared `liblbug.so` + rpath).

### Linux runtime tip

If the linked worker/binary expects `liblbug.so`, verify with `ldd` on the release binary and ensure `LD_LIBRARY_PATH` (or rpath) resolves.

## Run

Example (after `--init-schema` once per new `.lbug`):

```powershell
$env:JINA_API_KEY = "<your-token>"
cargo run --release -- `
  --repo D:\path\to\repo `
  --db D:\path\to\code.lbug `
  --repo-id my-repo `
  --init-schema `
  --dimensions 2048
```

Flags match `clap` definitions in `src/lib.rs`: `--batch-units`, `--jina-model`, `--jina-task`, optional `--scip`, etc.

## `CALLS` via rust-analyzer SCIP (optional, Rust v1)

Populate **`CALLS` (`Function → Function`)** edges by producing **`index.scip`** in the repo you index:

```powershell
Set-Location D:\path\to\repo
rust-analyzer scip .
```

Then either rely on auto-discovery (`index.scip` next to `--repo`) or pass **`--scip D:\path\to\index.scip`**. See **[`jina-embeddings.md`](../../jina-embeddings.md)** in this workspace for semantics and limitations.

## Embedding dimension

`--dimensions` must match the `Chunk.embedding FLOAT[N]` DDL created at `--init-schema`. Changing width later requires a new DB or migration. Maximum dense width for `jina-embeddings-v4` in this profile is **2048** (`schema_ddl::JINA_EMBED_DIM_V4_MAX`).
