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

Canonical sources live in **`src/repo_jina_lb/*.rs`** in this workspace. **`standalone-pack/`** is a copy-out template: after changing ingest logic, mirror the Rust files into **`standalone-pack/src/`** (`lib.rs` ↔ `mod.rs`). If the template is missing copies, **`cargo build` inside `standalone-pack` will not match this module** until you resync manually.
