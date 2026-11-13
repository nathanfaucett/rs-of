# Coordinator handoff after integration 52ac21c7b

Status: **full Playwright green; one real Vitest vault test blocked**. No Rust source, Cargo file, dependency alias, or local-auth assertion was changed by the E2E agent.

## Actual completed suites

Final runs use the integrated root executable `of/target/debug/unified-server`, selected from this worktree as `E2E_BACKEND_BIN=../../../target/debug/unified-server`. Each run starts a new process with empty temporary backend data.

- Full Playwright: **11 passed**, no skipped tests. Includes actual browser sign-in/consent/PKCE/callback/reload, both clients' remote Storage SQL CRUD, and the first Device returning `approved` and appearing in the Device list. The integration local-auth assertions are unchanged.
- Full serial backend Vitest: app **4 passed**; password-manager **4 passed, 1 failed**. Health, User info, Device list and Storage SQL writes pass in both browsers.
- Restored isolated frontend tests: **30 passed**. Lifecycle tests: **7 passed**. E2E TypeScript and browser SDK compilation pass.

The previous Storage create 500 and first-Device approval 404 reports are superseded. First enrollment is already approved; attempting to approve it again was a test error, fixed by local-auth integration.

## Remaining request: missing local WebSocket route

The strict encrypted-vault test now initializes the real WASM module, creates a remote database, creates local vault tables, encrypts a secret, and reads the local encrypted record through the real browser SDK. Its next public SDK operation is:

```ts
await handle.sync({ direction: "push" });
```

The SDK attempts `ws://127.0.0.1:1355/storage/sync/ws`. Browser error: `SyncError: Sync WebSocket failed`.

A separate real HTTP upgrade probe on a fresh integrated backend confirms:

```http
GET /storage/sync/ws HTTP/1.1
Host: 127.0.0.1:1355
Connection: Upgrade
Upgrade: websocket
Sec-WebSocket-Version: 13
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
```

**Actual: HTTP 404, empty body**, not 101. Repeated with the final root executable. This fails before any Hello, AttachRequest, bearer validation or SQL owner message can be sent. The browser client URL matches `storage-server/src/sync.rs::sync_router`. No fake socket, direct-HTTP replacement, skipped sync, or removed remote ciphertext assertion was introduced.

Required Rust-owner decision/repair: expose the supported authenticated SQL sync endpoint in the local composition, or provide its canonical local endpoint/contract if different. Do not infer that a mounted route alone will make the vault test pass: subsequent identity, policy, browser-WASM protocol and resource synchronization have not yet run. Direction handling is unchanged in the SDK (it currently requests bidirectional attach); it was not relaxed to mask this failure.

## Reproduce

Run serially from `.scratch/worktrees/e2e`:

```sh
E2E_BACKEND_BIN=../../../target/debug/unified-server pnpm test:e2e
E2E_BACKEND_BIN=../../../target/debug/unified-server pnpm test:backend
E2E_BACKEND_BIN=../../../target/debug/unified-server pnpm --filter @ofsdk/password-manager test:backend
```

Final logs: ignored `e2e/.scratch/integration52-playwright-confirmed.log`, `integration52-vitest-final.log`, and `integration52-websocket-final.log`.

## Binary provenance

Root HEAD: `52ac21c7b`. The coordinator had an uncommitted desktop Cargo.lock correction and a test-only IdP test repair. E2E did not modify either. The root binary was built normally with `cargo build --locked -p unified-server`; no Rust feature matrix or CRAP run was started by E2E. Initial root binary was absent; the first E2E-worktree locked build failed without changing its lockfile, and one root build attempt timed out before the successful build.

The coordinator subsequently rebuilt the same integrated root executable while running its Rust checks. Final Playwright, Vitest and repeated upgrade-probe runs used the resulting executable:

- Modified: 2026-11-13 13:02:13 -0500.
- SHA-256: `08f44e72b66304a5e528bf2de2093fe0fc3b3603324193b44b34d6ee7a8631db`.

No all-green Vitest or completed remote encrypted-vault CRUD claim is made. See [validation](runtime-validation.md).
