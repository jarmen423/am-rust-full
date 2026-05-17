# Install & extract (`jina-ladybug-repo-index`)

## Extract into its own repo

1. Copy the directory **`standalone-pack/`** (under `src/repo_jina_lb/`) to a new location, e.g.:

   ```powershell
   Copy-Item -Recurse "D:\code\agentic-memory\am-rust-full\src\repo_jina_lb\standalone-pack" "D:\code\jina-ladybug-repo-index"
   ```

2. Open that folder — it contains **`Cargo.toml`**, **`src/`**, **`README.md`**, **`.gitignore`**. **`install.md`** is not duplicated there; copy it from **`src/repo_jina_lb/install.md`** in this repo **or** use workspace **`jina-embeddings.md`**. Also copy any missing **`src/*.rs`** from **`src/repo_jina_lb/`** so the crate matches **`mod.rs`** upstream (see **`README.md`** sync note).

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

Flags match `clap` definitions in **`mod.rs`** (standalone: `src/lib.rs`): `--batch-units`, `--jina-model`, `--jina-task`, optional `--scip`, **`--force-reindex`**, etc.

### Incremental runs

- Lines **`ok  path`** vs **`skip path`** behave as in **`jina-embeddings.md`** (workspace root): **`skip`** means same MD5 + stored **`jina_fingerprint`** (model | task | dimensions); no Jina / no Chunk churn for that file.
- **`--force-reindex`** disables skipping.
- Older `.lbug` files without **`jina_fingerprint`** on `File` get a full ingest until completion writes it.

## `CALLS` via rust-analyzer SCIP (optional, Rust v1)

Populate **`CALLS` (`Function → Function`)** edges by producing **`index.scip`** in the repo you index:

```powershell
Set-Location D:\path\to\repo
rust-analyzer scip .
```

Auto-discovery looks **only** for the protobuf basename **`index.scip`** under `--repo`:
`index.scip` (repo root), `target/index.scip`, or `.scip/index.scip`.
If none match, stderr prints **`scip: skipped`** (no SILENT noop). Prefer **`--scip`** with an absolute path when in doubt.

Then either rely on that discovery or pass **`--scip D:\path\to\index.scip`**. See **[`jina-embeddings.md`](../../jina-embeddings.md)** in this workspace for semantics and limitations.

## Windows **`scip-python`** and merged multi-language SCIP

If you maintain the separate **`ladybug-jina`** workspace (**`build_merged_scip.ps1`**, **`patch_scip_python_windows.ps1`**, **`merge-scip`** cookbooks), use its **`install.md`** as the full reference:

- **Upstream**: global **`@sourcegraph/scip-python`** startup crash — **[Issue #210](https://github.com/sourcegraph/scip-python/issues/210)**; proposed source fix (**open PR**) — **[PR #211](https://github.com/sourcegraph/scip-python/pull/211)**. Prefer commenting / reviewing **#211** rather than filing duplicate bugs.
- **Scripts**: **`build_merged_scip.ps1`** accepts **`-LadybugJinaRoot`**. **`patch_scip_python_windows.ps1`** does **not** — only **`BundlePath`** / **`DryRunNode`**. Passing **`-LadybugJinaRoot`** to the patch script yields *parameter cannot be found*.
- Re-patch after each **`npm install -g @sourcegraph/scip-python`** ( **`dist/`** is overwritten).

## Embedding dimension

`--dimensions` must match the `Chunk.embedding FLOAT[N]` DDL created at `--init-schema`. Changing width later requires a new DB or migration. Maximum dense width for `jina-embeddings-v4` in this profile is **2048** (`schema_ddl::JINA_EMBED_DIM_V4_MAX`).
