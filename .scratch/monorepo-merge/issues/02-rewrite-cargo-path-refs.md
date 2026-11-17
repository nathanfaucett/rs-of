# Ticket 02: Rewrite Cargo path dependencies to in-repo locations

Status: ready-for-agent

Blocked by: 01-copy-remaining-ofdb-crates

## What it delivers

All Cargo manifests in the workspace resolve their path dependencies against `services/*`, `db/*`, `fs/*`, `net/*` — no `../ofdb/`, `../ofnet/`, `../offs/`, or `crates/*` references remain.

## Scope

Rewrite every `Cargo.toml`:

1. **Root `of/Cargo.toml`**
   - `members = ["services/*", "db/*", "fs/*", "net/*"]` (exclude `db/wasm`)
   - `unified-server = { path = "./services/unified-server" }`
   - `workspace.dependencies.setup = { path = "services/setup" }`
   - dev-deps: `crates/X` → `services/X`, `../ofnet/crates/iroh-chain` → `net/iroh-chain`, `../ofdb/crates/sql` → `db/sql`

2. **Each `services/*/Cargo.toml`** (11 files: `db`, `device-server`, `device-service`, `idp-server`, `idp-service`, `management-server`, `management-service`, `model`, `storage-client`, `storage-sdk`, `storage-server`)
   - `../../../ofdb/crates/X` → `../../db/X`
   - `../../../ofnet/crates/iroh-chain` → `../../net/iroh-chain`
   - `../../../offs/crates/file-system` → `../../fs/file-system`

3. **`apps/app/src-tauri/Cargo.toml`**
   - `../../../crates/X` → `../../../services/X`

4. **`fs/file-system/Cargo.toml`**
   - `../../../ofdb/crates/{platform,kv}` → `../../db/{platform,kv}`

5. **Moved crates' internal sibling paths** (e.g., `db/sql-engine-redb/Cargo.toml` `path = "../btree"`, `db/sql-server/Cargo.toml` `path = "../sql-engine"`, etc.) — already correct since all 32 crates now exist under `db/`.

## Acceptance

- `cargo metadata --format-version 1` succeeds with zero "path dependency does not exist" errors
- `cargo check --workspace` parses without resolution failures
- `grep -rn 'path = "\.\./' of/ --include='Cargo.toml' | grep -E 'ofdb|ofnet|offs|crates/'` returns nothing (except `db/wasm` which is excluded)
- Every `repository` field in moved crates points at the `of` repo
