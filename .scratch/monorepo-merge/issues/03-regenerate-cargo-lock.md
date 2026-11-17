# Ticket 03: Regenerate Cargo.lock for the merged workspace

Status: ready-for-agent

Blocked by: 02-rewrite-cargo-path-refs

## What it delivers

A single `of/Cargo.lock` that resolves all workspace members including the moved `db/*`, `fs/*`, `net/*` crates, so that CI's `--locked` builds succeed.

## Scope

- Run `cargo update` (or `cargo generate-lockfile`) from `of/` after Ticket 02 changes land
- Verify the lockfile contains entries for all moved crates (`ofdb-sql`, `ofdb-kv`, `ofdb-sync`, `iroh-chain`, `file-system`, etc.)
- Ensure `db/wasm` still produces its own lockfile (not in root lockfile)

## Acceptance

- `cargo check --workspace --locked` passes
- `grep -c 'ofdb-sql\|ofdb-kv\|ofdb-sync\|iroh-chain\|file-system' Cargo.lock` > 0
- `ls db/wasm/Cargo.lock` exists and is separate from root `Cargo.lock`
