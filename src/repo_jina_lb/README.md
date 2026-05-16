# `repo_jina_lb` — Jina → Ladybug code ingest

This directory holds two related layouts:

| Layout | Purpose |
|--------|---------|
| **`*.rs` here** (`mod.rs`, `parse.rs`, …) | Embedded module inside [`am-workspace`](../../Cargo.toml) (`pub mod repo_jina_lb`). Built with `--features jina-ladybug-index`. |
| **`standalone-pack/`** | Self-contained Cargo crate ready to **copy out**, `git init`, and publish as its own repo. |

## Making a separate git repository

1. Copy **only** the folder [`standalone-pack`](./standalone-pack/) to a new top-level directory (for example `jina-ladybug-repo-index`).
2. Rename optional — crate package name is already `jina-ladybug-repo-index`.
3. `cd` into that directory and run `cargo build --release`.
4. `git init`, add files, commit.

Details: [`install.md`](./install.md).

## Keeping workspace + standalone in sync

After changing Rust sources **here**, mirror the same edits into `standalone-pack/src/` (except `lib.rs`, which is a copy of `mod.rs`). Until this repo adopts a workspace member path crate, **manual sync** avoids drift.
