# Coordinator handoff: remaining real backend failures

Branch: `backend-simplification-e2e`; integration ancestor: `8596dfa08`.
No Rust implementation or dependency files were changed in this frontend pass.

## Runtime under test

Every suite starts a separate real HTTP backend on an empty temporary directory.
Executable: `../buildfix/target/debug/unified-server`, modified 2026-11-13 07:58:56 -0500.
SHA-256: `9462eae55a744634fdc908058f4043bf5a06c42b3b3ae1123a7cfcba9f9fe0c7`.
The buildfix worktree HEAD was `200c29056`, with uncommitted Rust owner changes. This executable is not a build of the E2E branch; the Rust agent owns its build qualification.

## Passing baseline

- All four owner health routes and real OIDC discovery.
- Administrator password sign-in and User info.
- Both seeded clients (`app`, `password-manager`): consent, S256 PKCE code exchange, and User info. Requests follow `crates/unified-server/tests/composition.rs::authenticate`.
- Actual app browser sign-in, protected route, session reload, Device list and Storage list access.
- Actual password-manager browser sign-in, server-owned consent metadata, denial with preserved state, approval, PKCE callback, verified client Application URI, User info, vault selector, and session reload.
- Device list after token exchange now returns 200 in both real Vitest browser suites. The previous 503 list blocker is superseded.

The old setup-page redirect, unsupported browser scopes, lost PKCE state, callback initialization loop, orphan Svelte effect, Node SDK import, database-name-as-ID request, Vite SQL WASM denial, and incorrect frontend Device audience are fixed on this branch. No fake setup evidence, injected session, HTTP interception, or disabled identity validation was used.

## 1. Storage creation: HTTP 500

Use either frontend's PKCE access token. The real SDK first succeeds with:

```http
POST /idp/oauth2/token
Content-Type: application/x-www-form-urlencoded

grant_type=urn:ietf:params:oauth:grant-type:token-exchange
subject_token=<frontend access token>
subject_token_type=urn:ietf:params:oauth:token-type:access_token
client_id=app
resource=storage:databases
authorization_details=[{"type":"storage","actions":["write"]}]
```

It then sends:

```http
POST /storage/databases
Authorization: Bearer <exchanged access token>
Content-Type: application/json

{"name":"e2e-app"}
```

Actual: **500, empty body**. Backend stderr:

```text
[ERROR storage_server::router::resources] Storage database creation failed: I/O error: No such file or directory (os error 2)
```

Also reproduced with `client_id=password-manager`, name `e2e-password-manager`, and with `local-login` in both Vitest browser suites. The encrypted vault's real sync SDK fails creation at the same endpoint. This replaces the earlier 503 report. Later SQL/vault CRUD and remote ciphertext assertions remain in place but cannot execute.

## 2. Device approval: HTTP 404 after successful enrollment

After app consent/PKCE, exchange the access token for `resource=http://127.0.0.1:1355/device`, `client_id=app`. Exchange succeeds. A generated Device client then sends:

```http
POST /device/devices/enrollments
Authorization: Bearer <exchanged access token>
Content-Type: application/json

{"name":"E2E browser device","publicKey":"<64 hex characters from a new Ed25519 public key>","address":"127.0.0.1:23456"}
```

Enrollment succeeds and returns a nonempty `id`. The next request uses that exact returned ID:

```http
POST /device/devices/enrollments/01a1d512-7972-7780-8b90-2daf0cda79c4/approve
Authorization: Bearer <same exchanged access token>
```

Actual: **404 `{"error":"not_found"}`**. No request body; this matches the generated `approveEnrollment({ deviceId: enrollment.id })` contract. Approval and approved-Device listing assertions are retained. The prior 503 enrollment blocker is superseded; enrollment now succeeds.

## Reproduce

Run serially from this worktree:

```sh
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm exec playwright test --config e2e/playwright.config.ts owners.e2e.ts
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm test:backend
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm exec playwright test --config e2e/playwright.config.ts app.e2e.ts password-manager.e2e.ts
```

Measured full Playwright run: **8 passed, 3 failed**. Real Vitest: app **3 passed, 1 failed**; password-manager **3 passed, 2 failed**. Browser-only frontend tests: app **3 passed**, password-manager **2 passed**, including reload.

Local logs: ignored `e2e/.scratch/playwright-final-current.log`, `vitest-browser-final.log`, `browser-app-final.log`, and `browser-password-manager-reload.log`. Runner reports are replaced by later runs. They contain ephemeral local-test credentials; do not publish raw traces without review.
