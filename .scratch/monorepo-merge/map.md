# Monorepo Merge — Map

## Notes

- Spec: `.scratch/monorepo-merge/spec.md`
- The folders `db/`, `fs/`, `net/`, `packages/sql|kv|net|file-system|example`, `services/` are already copied in `of/` but untracked.
- Only 7 of 32 ofdb crates were copied initially; Ticket 01 copies the remaining 25.
- `db/wasm` is a standalone wasm-pack workspace (excluded from Cargo workspace members, own lockfile) — keep it that way.
- `of/crates/` was renamed to `of/services/` on disk (git shows all `crates/*` files deleted, `services/*` untracked) — the Cargo workspace still references `crates/*`.
- Old repositories `ofdb`, `ofnet`, `offs` remain on disk and on GitHub for history; this effort is purely additive inside `of/`.

## Decisions-so-far

| Decision | Ticket |
|---|---|
| Layout: `ofdb/crates/*` → `of/db/*`, `ofnet/crates/*` → `of/net/*`, `offs/crates/*` → `of/fs/*`, TS wrappers → `of/packages/*`, `crates/*` → `services/*` | Spec |
| `db/wasm` excluded from workspace members (standalone) | Spec |
| Package names (`ofdb-*`, `iroh-chain`, `file-system`) and feature flags unchanged | Spec |
| Single root `GLOSSARY.md` absorbing all three old glossaries | Spec, Ticket 06 |
| CI: port ofdb test.yml (lint isolation, crap, browser matrix, native regression), ofnet wasm job, ofdb docker.yml | Spec, Ticket 07 |
| Docker context: single `of/` copy, drop ofdb/offs/ofnet COPY lines | Spec, Ticket 08 |
| Lockfiles regenerated after path rewrites (Cargo.lock, pnpm-lock.yaml) | Spec, Tickets 03, 05 |

## Fog

- **Duplicate `@ofsdk/sql` package**: `packages/sql` (wrapper) and `packages/sql-wasm` (checked-in wasm) both declare `"name": "@ofsdk/sql"`. `packages/sync-sdk` uses `@ofsdk/sql: workspace:*`. Not resolved in this spec — flagged, left as-is.
- **`db/wasm/Cargo.lock` vs root `Cargo.lock`**: need to ensure `cargo hack test --workspace` doesn't try to include `db/wasm` (excluded in members). The old ofdb workspace used `exclude = ["crates/wasm"]`; we'll mirror that.
- **ofdb browser test harness (`tests/browser/`)**: uses wasm-bindgen-test; porting it means pulling in the whole harness + test-fixtures + shell scripts. Scope confirmed in Ticket 07.
- **Lint table differences**: `of`'s `[workspace.lints]` is a superset of the old repos'; if a moved crate trips a new lint (e.g., `empty_structs_with_brackets`), we'll decide per crate rather than weakening the shared table.
- **`packages/example` dependencies**: currently `@ofsdk/kv` and `@ofsdk/sql` via `file:../kv` / `file:../sql` — these are already in-repo and should work once pnpm lockfile regenerated.
- **`ofdb` remote-feature isolation greps**: the exact grep patterns are in `ofdb/.github/workflows/test.yml` and must be ported verbatim to `of`'s lint job.
- **`justfile` recipe merging**: old repos have `browser-*`, `build-wasm`, `test-wasm`, etc. recipes; they must not conflict with existing `of/justfile` recipes.
