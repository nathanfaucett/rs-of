# Ticket 01: Copy remaining ofdb crates into of/db

Status: ready-for-agent

Blocked by: (none)

## What it delivers

All 32 ofdb crates exist under `of/db/` so that every Cargo path dependency in `services/*`, `tests/*`, and root `Cargo.toml` resolves locally without any external checkout.

## Scope

- Copy the 25 missing crates from `../ofdb/crates/` into `of/db/`:
  `btree`, `btree-automerge`, `btree-indexeddb`, `btree-redb`, `kv`, `kv-client`, `kv-client-cli`, `kv-server`, `kv-server-cli`, `kv-store`, `sql`, `sql-client`, `sql-engine`, `sql-engine-automerge`, `sql-engine-indexeddb`, `sql-history`, `sql-macros`, `sql-proto`, `sql-protocol`, `sql-query`, `sql-schema`, `sql-server-cli`, `sql-translator`, `sync`, `value`
- Keep `db/wasm` excluded from workspace members (standalone wasm-pack workspace, own lockfile)
- Preserve each crate's Cargo package name (`ofdb-*`, `iroh-chain`, etc.) and `version = "0.1"` so existing path deps keep resolving
- Fix the `repository` field in each copied crate's `Cargo.toml` to point at the `of` repository

## Acceptance

- `ls of/db/` shows all 32 directories
- Every `path = "../../../ofdb/crates/X"` in `services/*/Cargo.toml` and root `Cargo.toml` now resolves to a real directory
- `cargo metadata --format-version 1` parses without "path dependency does not exist" errors for ofdb crates
