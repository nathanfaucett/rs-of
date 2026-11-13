# Unified local WebSocket route and scoped authorization

Base: integration `6658fb000`, merged into the existing buildfix branch before this repair. Canonical sibling roots: ofdb `a15e05a`, offs `3087684`. No dependency path or alias changes. No merge back to integration/main.

## Rust changes

- Unified-server now enables Storage's `sync` feature, which includes `network`, so its actual local router mounts `/storage/sync/ws`.
- Local and independent deployments use the same typed WebSocket transport, native checked SQL history exchange, shared DatabaseRuntime, subject/application/resource namespace, and pre-apply authorization checks.
- Local admission uses the existing `authorize_storage_token` API. It requires a User bearer access token whose audience and resource are `storage:databases`, with both read and write actions for the bidirectional session. A user-info token, filesystem capability, client principal, expired token, or one-sided permission is rejected.
- Local WS authorization does not depend on a nonexistent Device selection for the local Storage transport endpoint. This follows local HTTP Storage's capability/namespace policy; it does not start mesh replication or weaken independent Device selection policy.
- Every frame and each atomic pre-apply guard re-evaluates authorization against the exact retained session scope. Changed subject/application/resource or withdrawn rights cannot continue the session.
- Existing checked resource-history exchange uses shared native persistence. The shared local route test verifies transfer, retirement, and absence of resource sidecars/staging files. The normal local-runtime integration test now asserts that the WS route is present when sync is enabled.

Risk: security-sensitive local/independent admission distinction. Reuse existing Storage capability validation and keep independent Device policy unchanged. No IdP token exchange fallback, wrong-audience bypass, or frontend/E2E edits.

## Passed validation

| Command | Result |
| --- | --- |
| `cargo build --locked -p unified-server` | Passed, canonical sibling dependencies |
| `cargo test -p storage-server --all-features --all-targets -- --test-threads=1` | **104 unit tests + 5 integration tests passed**; existing isolated HTTPS child entrypoint directly ignored, exercised by its parent |
| `cargo test -p storage-server --all-features --lib sync:: -- --test-threads=1` | **9 passed**, including shared persistence and local scoped admission |
| `cargo clippy -p storage-server --all-features --all-targets` | Passed with warnings |
| `cargo test -p unified-server --test composition -- --test-threads=1` | **3 passed**, fresh HTTP/HTTPS health/authentication/restart and serial startup deadline |
| `pnpm --filter @ofsdk/storage-sync-client test` | **114 passed** (57 source + 57 compiled tests) |
| `pnpm --filter @ofsdk/storage-sync-client fixtures:check` | Passed; browser wire fixtures match actual Rust frame/control types |
| Explicit Rust formatting and `git diff --check` | Passed |

No heavy workspace feature matrix or CRAP run was started in this pass, per task scope. The previous unrelated workspace client blockers are not repaired here.

## Actual browser result: not all green

Ran the real `@ofsdk/password-manager test:backend` twice against the newly built server, without changing tests or assertions:

1. From buildfix after enabling the route: **4 passed, 1 failed**. Error advanced from 404 to `Sync authorization rejected`.
2. From the frontend-owned `.scratch/worktrees/e2e` at `577ba2dce`, after the final scoped local runtime build: **4 passed, 1 failed**, same authorization rejection. No mocked backend or removed ciphertext/password/update/delete assertion.

The browser SDK currently passes the original OIDC User-info token directly to SyncClient:

- `packages/sync-sdk/src/browser.ts`, `BrowserDatabaseHandle.getSyncClient`: `tokenProvider: () => this.config.getToken()`.
- `apps/password-manager/src/test/vault.backend.test.ts`: `getToken` returns `login.access_token` from the original password login.

That token is not a SQL Storage capability. The frontend owner must use the existing IdP token exchange before WS attach, requesting resource `storage:databases` and authorization details `[{"type":"storage","actions":["read","write"]}]`, retaining the original subject and application. The existing TypeScript StorageClient already performs this exchange for HTTP SQL operations. Do not authorize user-info tokens directly or replace the shared token provider used by unrelated User-info operations with a resource token.

Remote encrypted ciphertext/update/delete and browser-WASM owner history compatibility remain **unqualified** because the unchanged vault test stops at attach authorization. The route/transport repair is not a claim that the vault test passed.

## Coordinator/Frontend handoff

Use this exact newly built executable:

`/home/nathan/code/rust/aicacia/of/.scratch/worktrees/buildfix/target/debug/unified-server`

SHA-256: `ed1f1dd0c26b3522d7cca890a3772d65acd77545c2cb26a5552051019f8518ff`.

From the E2E worktree:

```sh
E2E_BACKEND_BIN=/home/nathan/code/rust/aicacia/of/.scratch/worktrees/buildfix/target/debug/unified-server CHOKIDAR_USEPOLLING=true pnpm --filter @ofsdk/password-manager test:backend
```

Final browser log: `.scratch/worktrees/e2e/.scratch/local-ws-vault-final.log`. Rust and wire validation logs are untracked under buildfix `.scratch/local-ws-*`. Logs/screenshots can contain local test credentials; do not publish without review.

Only owned Rust WS/runtime/tests, unified feature flags, and this validation note are committed. No sibling root/worktree, storage-client crate, frontend source, or E2E assertion was changed.
