# Agent notes — standalone `jina-ladybug-repo-index`

## Purpose

Cargo crate at repo root: **Jina embeddings → Ladybug CODE_SCHEMA ingest**. Read [`README.md`](./README.md) for overview.

## Dependencies & secrets

- **`JINA_API_KEY`** — required for `api.jina.ai` embeddings OR `--jina-api-key`.
- **`lbug`** — requires correct Ladybug native libs for link/load (Windows: shared DLL workflow; see **`install.md`** and upstream `lbug-crate-windows.md` where applicable).

Never commit `.env`, tokens, or `.lbug` files.

## Schema coupling

`schema_ddl.rs` mirrors Agentic Memory Python `CODE_SCHEMA`. If upstream adds columns or labels, update DDL + writers together.

## Operational hints

- Run `--init-schema` once per fresh DB (embedding dimension is baked into `Chunk.embedding FLOAT[N]`).
- Prefer conservative `--batch-units` if Jina returns rate limits.
- Per-file errors print to stderr as `ERR <path>: …`; exit status still 0 unless CLI parse fails — intentional for batch walks.
