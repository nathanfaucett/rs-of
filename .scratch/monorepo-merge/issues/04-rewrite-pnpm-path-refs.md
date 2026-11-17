# Ticket 04: Rewrite pnpm package.json paths and port build-sql-wasm.mjs

Status: ready-for-agent

Blocked by: 01-copy-remaining-ofdb-crates

## What it delivers

All TypeScript packages build their wasm artifacts from the in-repo crates; `apps/password-manager` consumes the workspace `@ofsdk/sql`; `pnpm install --frozen-lockfile` succeeds.

## Scope

1. **`packages/kv/package.json`**: `build:wasm` `cd ../../crates/wasm` → `cd ../../db/wasm`
2. **`packages/net/package.json`**: `build:wasm` `cd ../../crates/iroh-chain-wasm` → `cd ../../net/iroh-chain-wasm`
3. **`packages/file-system/package.json`**: `build:wasm` `cd ../../crates/file-system-wasm` → `cd ../../fs/file-system-wasm`
4. **Port `../ofdb/scripts/build-sql-wasm.mjs` → `of/scripts/build-sql-wasm.mjs`** and update its `crates/wasm` argument to `db/wasm`
5. **`packages/sql/package.json`**: `build:wasm` calls the new local script (already `../../scripts/build-sql-wasm.mjs`)
6. **`apps/password-manager/package.json`**: `"@ofsdk/sql": "link:../../../ofdb/packages/sql"` → `"@ofsdk/sql": "workspace:*"` (or `link:../../packages/sql`)
7. **`pnpm-workspace.yaml`**: remove `crates/iroh-client-wasm` (never existed); change `crates/tauri-plugin-fetch-api` → `services/tauri-plugin-fetch-api`
8. **Root `package.json`**: `packages:build` filter `./crates/*` → `./services/*`
9. **`packages/sql/browser-tests/package.mjs`**: `file:../ofdb-sql-0.1.0.tgz` path update

## Acceptance

- `pnpm install --frozen-lockfile` succeeds from `of/`
- `pnpm --filter @ofsdk/sql run build:wasm && pnpm --filter @ofsdk/sql run build:esm` produces artifacts under `packages/sql/pkg/` and `packages/sql/esm/`
- `pnpm --filter @ofsdk/kv run build:wasm`, `pnpm --filter @ofsdk/net run build:wasm`, `pnpm --filter @ofsdk/file-system run build:wasm` succeed
- `pnpm sdk:build` and `pnpm e2e:prepare` pass
- `grep -r 'ofdb/packages' apps/ packages/ --include='package.json'` returns nothing
