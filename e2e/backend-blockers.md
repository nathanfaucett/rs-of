# Coordinator handoff: real backend failures

Integration merged: `8596dfa08`. Executable: the real `../buildfix/target/debug/unified-server`, built by the Rust agent. All checks below use an empty temporary data directory and HTTP `127.0.0.1:1355`, not a reused backend. No Rust or frontend implementation was edited.

## Passing baseline

- All four `/{idp,device,storage,management}/health` routes return 200.
- `POST /idp/oauth2/token` with form `grant_type=password`, `client_id=local-login`, `username=admin@local.test`, `password=password`, `scope=openid profile email` returns 200.
- Bearer `GET /idp/userinfo` returns the administrator email.
- For **both** `app` and `password-manager`: bearer `POST /idp/oauth2/auth/approve-for-user`, bearer `POST /idp/oauth2/auth` with S256 PKCE, and form `POST /idp/oauth2/token` authorization-code exchange all return 200. The resulting client token also gets User info successfully. These are separate passing Playwright tests, not an inference from a failed CRUD test.
- Reference: `crates/unified-server/tests/composition.rs::authenticate`. E2E uses the same request sequence, with its configured `127.0.0.1` origins and a fresh random PKCE verifier.

## Rust owner endpoints: repair required

### Storage create

Use either frontend's PKCE access token from the baseline. The actual Storage SDK exchanges it with:

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

That exchange succeeds. Then the actual SDK sends:

```http
POST /storage/databases
Authorization: Bearer <exchanged access token>
Content-Type: application/json

{"name":"e2e-app"}
```

**Actual: 503, empty response body.** Expected: successful resource creation. Reproduced with `client_id=password-manager` / `{"name":"e2e-password-manager"}`. The password-manager real sync SDK also fails its create operation before vault CRUD.

### Device list and enrollment

After a successful baseline login (or app PKCE session), exchange the access token:

```http
POST /idp/oauth2/token
Content-Type: application/x-www-form-urlencoded

grant_type=urn:ietf:params:oauth:grant-type:token-exchange
subject_token=<access token>
subject_token_type=urn:ietf:params:oauth:token-type:access_token
client_id=local-login
resource=http://127.0.0.1:1355/device
```

Exchange returns 200. `GET /device/devices` with its bearer token returns **503 `{"error":"temporarily_unavailable"}`**. Reproduced by real Chromium in both Vitest apps.

Using an `app` PKCE token with `client_id=app` in the same exchange also returns 200. The generated Device SDK then sends:

```http
POST /device/devices/enrollments
Authorization: Bearer <exchanged access token>
Content-Type: application/json

{"name":"E2E browser device","publicKey":"<64 hex characters from a new Ed25519 public key>","address":"127.0.0.1:23456"}
```

**Actual: 503 `{"error":"temporarily_unavailable"}`**. Approval and listing assertions remain in the test; they cannot execute until enrollment works. These are single owner prefixes, not doubled-route mistakes.

## Frontend routing: implementation owner required

The real app browser navigates from `/signin` to `/setup/installation`. No Username field exists there and no password token request occurs. Password-manager's real OIDC client reaches app `/authorize?...`, which then also becomes `/setup/installation` instead of sign-in/consent.

Source inspection: `apps/app/src/lib/common/state/onboarding.ts` still requires independent setup owner evidence and a `login_client` from setup status. The new local backend deliberately has no setup routes. This is an implementation mismatch, not an E2E environment URL typo. The test must not supply fake setup evidence or inject a session. The exact browser request URLs and response bodies are retained in Playwright traces.

Password-manager also requests hard-coded `openid profile email storage:read storage:write`; the local seed allows `storage` instead. This scope mismatch has not yet been reached by the browser consent flow. The OIDC navigation itself reaches the correct app UI; the previous source-only suspicion that discovery navigation stops at the JSON API is superseded by this runtime observation.

## Reproduction

From this E2E worktree, run one command at a time:

```sh
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm exec playwright test --config e2e/playwright.config.ts owners.e2e.ts
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm --filter @ofsdk/app test
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm --filter @ofsdk/password-manager test:backend
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm exec playwright test --config e2e/playwright.config.ts app.e2e.ts password-manager.e2e.ts
```

The launcher starts and tears down a separate real process for each run. Local diagnostic logs are in `e2e/.scratch/`; Playwright traces are in `.scratch/e2e-results/` (each run replaces that runner's previous report). They contain ephemeral local-test tokens, not production credentials.
