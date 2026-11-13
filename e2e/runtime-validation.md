# Ticket05 validation against integration 52ac21c7b

Branch: `backend-simplification-e2e`; worktree `.scratch/worktrees/e2e`.
The worktree was clean before integration merged by fast-forward to `52ac21c7b`. Nothing is merged back into integration.

**Result: all 11 Playwright tests pass. Both real-backend Vitest suites executed; 8 tests pass and one encrypted-vault test remains blocked by a missing local WebSocket route. This is not an all-green ticket05 result.**

## Owned fixes

- Set `CHOKIDAR_USEPOLLING=true` in the shared test-runner environment. The first actual frontend startup failed with `ENOSPC: System limit for number of file watchers reached`. This setting is test-only; production frontend configuration is unchanged.
- Give the Vitest Storage SQL fixture a primary key and use it in the insert. The old no-primary-key schema failed with HTTP 400 `executionFailed`; the corrected fixture performs real create/write/read/delete successfully in both browsers. This matches the working Playwright fixture and owner SQL contract.
- Fix the browser sync SDK at its existing raw-WASM boundary: initialize WASM in `createSync`, encode bound primitive parameters with the owner's tagged value format, and decode the final statement's tagged result cells into the existing `QueryResult` interface. Preserve large integer values as bigint. Unsupported parameter objects/non-integer numbers fail explicitly rather than being sent as invalid owner values. Rebuilt the tracked browser JS/declaration map with the package's existing compiler command.
- Keep all integration local-auth assertions, especially first enrollment `state === "approved"` followed by Device listing. Do not approve an already-approved Device again.
- Keep all encrypted-vault remote ciphertext, password-rejection, update and delete assertions. The remaining error is not hidden with a mock, test skip, local-only replacement or relaxed expectation.

Complexity/risk: small test-environment/schema fixes and a browser SDK codec/lifecycle correction. The SDK boundary change affects browser SQL callers; the real vault now reaches its WebSocket stage instead of failing on uninitialized WASM or plain bound parameters. No Rust source, Cargo file, dependency alias, independent desktop code, Svelte component, or CI workflow was edited in this pass.

## Final measured checks

Every backend suite used the root integrated prebuilt binary via `E2E_BACKEND_BIN=../../../target/debug/unified-server`. Each suite created a fresh temporary data directory and process. Full runs were serial, not test collection.

| Command                                           | Result                                                                                                                                                                                                                                                       |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `pnpm test:e2e` with the binary above             | **11 passed, 0 failed/skipped**, final complete run in 2.0 minutes. Covers both browser flows, consent denial/state, registration error UI, protected session/reload, both clients' PKCE and remote Storage SQL CRUD, and first-Device autoapproval/listing. |
| `pnpm test:backend` with the binary above         | Both real Chromium suites completed serially. App: **4 passed**. Password-manager: **4 passed, 1 failed**. The aggregate command correctly exits 1. Failing case: encrypted vault reaches `handle.sync` and receives a WebSocket transport failure.          |
| Fresh-process WebSocket upgrade probe             | **404, empty body** at `GET /storage/sync/ws`. Repeated against the final binary. No owner handshake can begin.                                                                                                                                              |
| `pnpm --filter @ofsdk/app test`                   | **30 passed**: native capabilities 7, registration/errors 13, explicit browser configuration/desktop isolation 10.                                                                                                                                           |
| `pnpm test:e2e:lifecycle`                         | **7 passed**: data isolation, readiness/timeout, occupied port, spawn/exit errors, explicit executable and teardown.                                                                                                                                         |
| `pnpm test:e2e:check`                             | Passed.                                                                                                                                                                                                                                                      |
| `pnpm --filter @ofsdk/sync-sdk run build:browser` | Passed. Tracked browser output rebuilt.                                                                                                                                                                                                                      |
| `git diff --check`                                | Passed.                                                                                                                                                                                                                                                      |
| Post-run sockets                                  | No listeners on 1355, 5173 or 5174 after completed suites and the timed-out attempt.                                                                                                                                                                         |

An initial full Playwright run after the watcher fix also passed all 11 tests in 46.6 seconds. A later confirmation attempt timed out at the external 240-second bound: it observed a Device 503 and subsequent request timeouts while other Rust validation was active. The cause of that transient is not proven. It was not masked by retries or test changes; the final full run passed all 11 unchanged assertions with a 360-second outer command bound. Per-test timeout remains unchanged. Raw attempt logs are retained.

## Backend provenance/build limits

The preferred root executable did not exist initially. The E2E-worktree `cargo build --locked -p unified-server` failed because the integrated lockfile needed the coordinator's desktop dependency correction. E2E left Cargo.lock unchanged. A bounded root build attempt timed out; the subsequent normal locked root backend build passed. No E2E Rust matrix or CRAP build was started.

Root HEAD remained `52ac21c7b`; the coordinator owned the uncommitted desktop lock correction and a test-only IdP test repair. Root Rust validation rebuilt the executable before final browser runs. Final binary modified 2026-11-13 13:02:13 -0500; SHA-256 `08f44e72b66304a5e528bf2de2093fe0fc3b3603324193b44b34d6ee7a8631db`. The first completed Playwright run used the earlier same-root build, SHA-256 `5bf443e22bdc1a9616fa52c896332285fd075085f4f5ea1d7b1eb20b19c558a1`.

## Remaining work and limits

1. The Rust local composition must expose a supported SQL synchronization endpoint before the vault test can verify remote ciphertext and complete encrypted CRUD. Exact failing request and reproduction commands are in [backend-blockers.md](backend-blockers.md). No Rust endpoint repair was attempted.
2. Later sync policy, identity, browser-WASM protocol, wrong-password-after-sync and encrypted update/delete behavior remain unqualified. Mounting a route is not proof these stages pass.
3. The earlier Storage create 500 and redundant first-Device approval 404 are superseded: both clients' real remote SQL CRUD and first-Device autoapproval/listing now pass.
4. Hosted CI, browser HTTPS, desktop runtime, independent multi-node setup and broad Rust checks were not run by this frontend pass. The previous password-manager offline-unit/type-check failures were not repaired or removed; no new clean full-frontend-unit/type-check claim is made.
5. Existing browser token local-storage persistence/security and explicit Application URI requirements remain as documented in the README.

Logs are preserved under ignored `e2e/.scratch/`: `integration52-playwright-confirmed.log`, `integration52-vitest-final.log`, `integration52-websocket-final.log`, plus earlier build/failure attempts. Runner traces/screenshots contain local-test credentials and must be reviewed before publication. Task artifact and worktree directories remain in place.
