# 01: Synchronize encrypted browser vaults through Storage

**What to build:** A signed-in password-manager User can synchronize an existing encrypted IndexedDB vault through the public sync SDK to its real Storage Resource. Initial secrets, updates, and deletions arrive as ciphertext through SQL history v7; plaintext and the vault master password never reach Storage. The work includes the browser v7 binding, the independent IndexedDB committed-change repair for legacy direct-peer synchronization, readable synchronization errors, focused tests, and current documentation. It does not reset existing local data or change the application's public synchronization workflow.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: resolved
Milestone: M2 — Single-Node encrypted synchronization
Dependencies: Existing SQL history v7 sessions, IndexedDB SQL persistence, Storage WebSocket synchronization, and real-backend browser runners. No new external service or synchronization package.

## Acceptance criteria

- [x] Existing IndexedDB vault data opens without deletion, reinitialization, or a storage migration; local-only vault behavior and OIDC sign-in remain unchanged.
- [x] A public browser SQL operation uses the existing v7 history session with caller-supplied byte transport and bounded in-memory staging. It does not send legacy v1 messages to Storage or require native filesystem/Tokio support.
- [x] Two independent IndexedDB Databases converge create, update, and delete history in both directions through the public v7 operation. Repeated synchronization does not duplicate Logical Rows.
- [x] IndexedDB retains the engine's existing committed-change signal, and the existing legacy direct-peer browser synchronization path passes a focused public-API regression test.
- [x] Sync SDK Storage call sites select the v7 operation while the public Database handle `sync` workflow remains unchanged. Success is reported only after a complete exchange; failures preserve the SDK error state.
- [x] The real-backend vault scenario creates and synchronizes an encrypted secret, then proves through the public Storage API that one remote ciphertext row exists and neither plaintext nor the master password is exposed.
- [x] The same scenario rejects a wrong master password, reopens with the correct password, and decrypts the synchronized local record successfully.
- [x] After a vault update and synchronization, remote ciphertext changes and the updated plaintext remains absent from Storage.
- [x] After a vault deletion and synchronization, the remote query returns no secret rows. Repeated synchronization remains idempotent.
- [x] Representative WASM-like errors retain their exposed message and original cause through the SQL error wrapper. No Token, plaintext, ciphertext payload, or arbitrary frame logging is added.
- [x] SQL WASM/browser tests, sync SDK checks, affected password-manager checks, focused Rust checks, formatting, and appropriate feature checks pass with recorded counts.
- [x] The root real-backend Vitest run passes both application suites against the rebuilt unified server. The real-backend Playwright suite passes without mocks, skipped synchronization, or local-only substitutes.
- [x] Current documentation describes only the qualified single-Node behavior. GitHub Actions remains the source of pushed qualification evidence.

## Non-goals

- Two-Device offline convergence, continuous real-time delivery, durable replay, or multihop synchronization.
- Device, Selection, Management, or Storage authorization redesign.
- Push-only/pull-only enforcement; the existing Storage session remains Bidirectional.
- KV or filesystem synchronization.
- A v1 adapter on the v7 route, a new wire version, or a vault/data migration.
- Native Node, independent deployment, desktop, or Tauri qualification.
- Production password-manager qualification or unrelated application features.

## Current evidence and limits

Implemented the public browser SQL history v7 operation with bounded in-memory staging, retained and repaired the legacy committed-change path, switched Storage SDK call sites to v7, preserved the public `DatabaseHandle.sync` workflow, and fixed WASM error wrapping. Review fixes now publish legacy events only after confirmed durable IndexedDB commits, defer WASM frees during sync settlement, reject oversized/non-byte frames before allocation, and bound attach/receive lifecycle. Both SDK implementations close attached sessions in `finally` and abort/await active synchronization on close. The public history operation uses `JsSqlSyncTransport` and internal `SessionConfig::default()`; the unused history config and duplicate adapter are removed.

The real password-manager flow synchronizes create, update, delete, and repeated retries as ciphertext without exposing plaintext or the master password. Existing IndexedDB names, schemas, and retained history are unchanged. Storage authorization and execution-time checks are unchanged.

The required real-backend Vitest and Playwright suites pass. Focused public IndexedDB, legacy sync, commit rejection, handle lifetime, frame bound, SDK lifecycle, Rust, feature, and application checks pass with the counts below. This qualifies only the explicit single-Node Chromium workflow. It does not qualify two-Device convergence, continuous delivery, durable replay, other browser engines, or production password-manager use.

## Comments

### 2026-11-21 — Implementation and local qualification

- `pnpm --filter @ofsdk/sql run build:all` — passed; rebuilt WASM, declarations, ESM, and browser artifacts from the changed Rust sources before browser validation.
- `SQL_TEST_PAGE=/browser-tests/history-sync.html BROWSER_PATH=/snap/bin/chromium pnpm --filter @ofsdk/sql exec playwright test` with `packages/sql/browser-tests/{bindings,history-sync,legacy-sync,lifecycle,migration-validation,migrations,parameters,row-identities,select-transport,select-values,select,sync-error,transaction-select}.test.ts` expanded as individual arguments — **50 passed**. This includes public v7 bidirectional create/update/delete/reopen/repeat, legacy committed-change, and message/cause/no-payload-logging tests.
- `pnpm --filter @ofsdk/sql check` and `pnpm --filter @ofsdk/sql run lint` — passed.
- `pnpm --filter @ofsdk/sync-sdk run build:browser`, `pnpm --filter @ofsdk/sync-sdk run build:node`, and `pnpm --filter @ofsdk/sync-sdk run build` — passed. `pnpm --filter @ofsdk/sync-sdk test` — **3 passed**. Direct Biome lint of `src/browser.ts` and `src/node.ts` completed with only four pre-existing useless-constructor infos in unrelated KV/filesystem handles.
- `pnpm --filter @ofsdk/password-manager test` — **174 passed, 5 skipped** after one rerun required by a Vite dependency-optimizer reload. `pnpm --filter @ofsdk/password-manager check` — **0 errors, 0 warnings**. Direct Biome lint of the two changed application files passed.
- `cargo build --locked -p unified-server && E2E_BACKEND_BIN=./target/debug/unified-server pnpm test:backend` — **9 passed**: app **4**, password-manager **5**. The password-manager scenario includes repeated create/update/delete sync, changed ciphertext, idempotency, wrong-password rejection, correct-password reopen/decrypt, and plaintext/master-password absence.
- `E2E_BACKEND_BIN=./target/debug/unified-server pnpm test:e2e` — **11 passed** against the rebuilt unified server.
- `cargo test -p ofdb-sql-engine` — **3 passed**. `cargo test -p ofdb-sql-engine-automerge` — **34 passed**. `cargo test -p ofdb-sql-history` — **16 passed**. `cargo test -p ofdb-sql --features validated-history,sql,in-memory` — **14 passed, 7 pre-existing ignored**. `cargo test -p of-storage-client --test websocket_sync` — **3 passed**. `RUST_MIN_STACK=67108864 cargo test -p storage-server --features sync ws_` — **9 passed**.
- `cargo hack check --feature-powerset --depth 1 -p ofdb-sql --exclude-features postgres,remote --all-targets` — **13 configurations passed**, including `wasm`. `cargo hack check --feature-powerset --depth 1 -p ofdb-sql-engine -p ofdb-sql-engine-automerge --no-dev-deps --lib` — **6 configurations passed**.
- Focused Clippy completed for `ofdb-sql-engine`, `ofdb-sql-engine-automerge`, `ofdb-sql-history`, and `ofdb-wasm`; only pre-existing warnings remain. Targeted LLVM coverage plus `cargo crap` completed and reported **48/483 functions above CRAP threshold 30**. This is a complexity finding, not a clean-CRAP claim.
- `cargo fmt --all -- --check`, `just docs-check`, and `git diff --check` — passed.

Qualification limits:

- The unmodified SQL package page references absent base helpers under `tests/browser/src`. The two unrelated fault-injection files `transactions.test.ts` and `migration-outcomes.test.ts` were not run; the other 13 browser files, including all new synchronization tests, passed using the real-WASM `history-sync.html` page. Do not infer a full `pnpm --filter @ofsdk/sql test` pass.
- The full sync-sdk and password-manager `biome lint .` scripts stop on pre-existing nested root configurations in `packages/kv` and `packages/sql`. Changed files were linted directly. Full workspace Clippy/CRAP and GitHub Actions were not run.
- An unrestricted SQL feature-powerset attempt stops at the pre-existing PostgreSQL-only `crate::database` import failure. The engine no-default-feature test target also has a pre-existing missing `vec!` import; relevant facade and library-only feature checks passed instead.
- A strict focused Clippy `-D warnings` attempt stops on established dependency-version and legacy SQL lint debt. Focused non-denying Clippy completed after the new doc warning was fixed. No unrelated lint cleanup is claimed.

### 2026-11-21 — Consolidated review fixes

- `pnpm --filter @ofsdk/sql run build:all` — passed after review fixes; regenerated WASM, declarations, ESM, and browser artifacts.
- `SQL_TEST_PAGE=/browser-tests/history-sync.html BROWSER_PATH=/snap/bin/chromium pnpm --filter @ofsdk/sql exec playwright test` with 14 selected files — **54 passed**. New regressions cover rejected commit events, deferred-receive close for legacy and history, and early oversized/non-byte frame rejection. The selected files are `bindings`, `history-sync`, `legacy-sync`, `lifecycle`, `migration-validation`, `migrations`, `parameters`, `row-identities`, `select-transport`, `select-values`, `select`, `sync-error`, `sync-lifetime`, and `transaction-select`.
- `pnpm --filter @ofsdk/sql check` and `pnpm --filter @ofsdk/sql run lint` — passed. Changed SQL test/source files also passed direct Biome lint.
- `pnpm --filter @ofsdk/sync-sdk run build:browser`, `pnpm --filter @ofsdk/sync-sdk run build:node`, and `pnpm --filter @ofsdk/sync-sdk test` — passed; **7 tests total**, including stalled-receive and close-during-sync coverage for both browser and Node implementations. Changed lifecycle files passed direct Biome lint with only the four pre-existing unrelated constructor infos.
- `cargo test -p ofdb-sql-engine` — **3 passed**. `cargo test -p ofdb-sql-engine-automerge` — **34 passed**. `cargo test -p ofdb-sql-history` — **16 passed**. `cargo test -p ofdb-sql --features validated-history,sql,in-memory` — **14 passed, 7 pre-existing ignored**. `cargo test -p of-storage-client --test websocket_sync` — **3 passed**. `RUST_MIN_STACK=67108864 cargo test -p storage-server --features sync ws_` — **9 passed**.
- `cargo check -p ofdb-btree-indexeddb --target wasm32-unknown-unknown` and `cargo check -p ofdb-sql-engine-indexeddb --target wasm32-unknown-unknown` — passed. `cargo hack check --feature-powerset --depth 1 -p ofdb-sql --exclude-features postgres,remote --all-targets` — **13 configurations passed**. `cargo hack check --feature-powerset --depth 1 -p ofdb-sql-engine -p ofdb-sql-engine-automerge --no-dev-deps --lib` — **6 configurations passed**.
- Focused Clippy completed for the changed SQL engine, history, browser persistence, and WASM crates; only pre-existing warnings remain. `cargo fmt --all -- --check`, `just docs-check`, and `git diff --check` — passed.
- `pnpm --filter @ofsdk/password-manager test` — **174 passed, 5 skipped**. `pnpm --filter @ofsdk/password-manager check` — **0 errors, 0 warnings**.
- `E2E_BACKEND_BIN=./target/debug/unified-server pnpm test:backend` — **9 passed**: app **4**, password-manager **5**. `E2E_BACKEND_BIN=./target/debug/unified-server pnpm test:e2e` — **11 passed** against the rebuilt unified server.

Remaining qualification limits:

- `transactions.test.ts` and `migration-outcomes.test.ts` remain excluded because the base page references absent helpers under `tests/browser/src`. The other 14 browser files pass with real WASM. Do not infer a full `pnpm --filter @ofsdk/sql test` pass.
- The test harness has no reliable caller-future cancellation seam for the legacy event stream. Issued-commit completion is owned by the existing IndexedDB completion task, but no separate cancellation/completion qualification is claimed. Rejected commits are covered directly.
- Full workspace Clippy/CRAP and GitHub Actions were not run. Full sync-sdk and password-manager `biome lint .` remain blocked by pre-existing nested root configurations; changed files were linted directly.
- An unrestricted SQL feature-powerset attempt retains the pre-existing PostgreSQL-only `crate::database` import failure and the engine no-default-feature test target's missing `vec!` import. Relevant facade and library-only feature checks passed instead.

### 2026-11-21 — Dist packaging and initialization race fixes

- Added `dist/sql-sync-lifecycle.js`, `dist/sql-sync-lifecycle.d.ts`, and `dist/sql-sync-lifecycle.d.ts.map` to the tracked dist convention. The ignore rule requires an explicit force-add for these generated files.
- `pnpm --filter @ofsdk/sync-sdk exec vitest run tests/dist-entrypoints.test.ts` — **2 passed**, importing the actual `dist/browser.js` and `dist/node.js` entry points and their relative lifecycle artifact. Hiding the lifecycle file first reproduced both missing-module failures before regeneration.
- Lifecycle/client construction is now synchronous. Immediate close cannot attach after close, and concurrent sync calls share one lifecycle/session. New tests pass for both implementations.
- The SQL close-lifetime tests now also prove IndexedDB deletion is incomplete while receive is pending and succeeds after operation settlement and handle release. Both history and legacy paths pass.
- Full affected validation: SQL browser **54 passed**; sync SDK **13 passed** across **3 files**, including 8 lifecycle tests and 2 dist import smoke tests; real-backend Vitest **9 passed** and Playwright **11 passed**. Direct changed-file Biome lint, `cargo fmt --all -- --check`, `just docs-check`, and `git diff --check` passed.
- This pass changes only TypeScript/tests/generated dist output, so no additional Rust test run was required beyond the focused Rust evidence above. Existing harness, full-workspace, nested-Biome, and caller-future-cancellation qualification limits remain unchanged.
