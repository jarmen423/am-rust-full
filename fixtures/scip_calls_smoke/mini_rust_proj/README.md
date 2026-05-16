# Mini Rust project — SCIP → `CALLS` smoke fixture

Sanity-check **`CALLS`** after **rust-analyzer SCIP** + **Jina** ingest.

## Generate `index.scip`

Requires **rust-analyzer** on `PATH`:

```powershell
Set-Location "D:\code\agentic-memory\am-rust-full\fixtures\scip_calls_smoke\mini_rust_proj"
rust-analyzer scip .
```

This writes **`index.scip`** in the project root (`rust-analyzer scip --help` for `--output` and other flags).

## Run the indexer

```powershell
$env:JINA_API_KEY = "…"
.\target\debug\jina-ladybug-repo-index.exe `
  --repo "D:\code\agentic-memory\am-rust-full\fixtures\scip_calls_smoke\mini_rust_proj" `
  --db "D:\temp\smoke_calls.lbug" `
  --repo-id "fixture/scip-calls-smoke" `
  --init-schema
```

## Verify in Ladybug

```cypher
MATCH (:Function)-[r:CALLS]->(:Function)
RETURN count(r) AS calls;
```

Expect a **non-zero** count when SCIP emitted `relationships` and definition lines match the tree-sitter walk.
