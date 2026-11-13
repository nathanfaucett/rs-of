# Local owner authentication validation

Branch: `backend-simplification-local-auth`, from integration `8596dfa08`.

## Scope and risk

Small changes to local client seeding, bearer audience validation, permission target validation, and local Storage content creation. Authentication changes have medium security risk and blocker priority. Tests use real owner HTTP boundaries, not resolver mocks or authentication bypasses.

No desktop implementation or dependency paths were edited. Device security-history and Storage mesh implementations were not rewritten. The committed security-history prerequisite `200c29056` was merged from its owner branch. Integration `backend-simplification` was merged into this branch; no changes were merged back.

## Root causes

- Local service clients were registered for Basic authentication, while the configured clients send `client_secret_post`. Register the method actually used; secret, scope, audience and current identity checks remain active.
- Device bearer middleware required the Management audience instead of the configured Device API audience. Require an exact Device audience and a User principal.
- The local administrator seed omitted `DevicesEnroll`. Add that explicit grant only to the local administrator.
- Enrollment approval uses an exact Device target, but the permission contract rejected that target for `DevicesEnroll`. Accept a non-nil Device ID only in installation scope.
- SQL-backed Storage catalogs do not create content directories as file catalogs did. Database creation now creates its content parent directory itself and retains catalog cleanup on failure.
- First-Device enrollment already returns `approved`. The E2E test incorrectly approved it again, which correctly returns 404 for a non-pending enrollment. Assert the returned approved state and list the Device instead. No enrollment security behavior was changed.

## Passing validation

- TDD: reproduced the original owner failures through the real E2E SDK requests. Added a process-level HTTP regression; observed service-client `invalid_client`, Device audience 401, enrollment permission 403, and Storage directory 500 before their fixes.
- `cargo test -p unified-server --test composition -- --test-threads=1 --nocapture`: **3 passed**. Covers fresh HTTP/HTTPS, seeded administrator and additional non-administrator User login, app/password-manager consent and PKCE, local owner service tokens, JWKS retrieval, Device enrollment/list/restart retention, Storage create/write/read, denied anonymous Device requests, denied SQL writes using read-only tokens, retained TLS certificate and restart.
- `cargo test -p device-server --lib device_authorization`: **1 passed**.
- `cargo test -p device-server --lib device_authority`: **7 passed**.
- `cargo test -p of-model --features replica,axum enrollment_approval_permission`: **1 passed**.
- `cargo clippy -p unified-server -p device-server -p idp-server -p storage-service -p storage-server --lib`: passed, existing warnings remain.
- Targeted `rustfmt --check` and `git diff --check`: passed.
- Current E2E-worktree Storage SDK tests against this branch's executable: **2 passed** for app/password-manager CRUD.
- Current E2E-worktree owner API suite, with only the first-Device assertion corrected: **6 passed** against this branch's executable, fresh temporary data, real HTTP and seeded end-user sessions. A temporary API-only Playwright configuration avoided concurrent frontend port use. The temporary spec reused the current E2E worktree's SDK/session/launcher imports; it was removed after validation. The corresponding assertion correction is committed in `e2e/owners.e2e.ts`.

## Incomplete broad checks

- `cargo hack test --feature-powerset --workspace --all-targets`: timed out after 120 seconds during compilation (matrix begins with 1/200). No complete matrix result.
- `just crap`: timed out after 120 seconds during compilation. No CRAP score was produced.
- All-target Clippy is blocked by unrelated Storage test imports (`ofdb_sql::SyncTransport`, `KvAuthorizedTransport`) and Device replication test partial-read errors. Library Clippy passes.
- An earlier parallel composition run exceeded the three-second startup deadline under concurrent build/test load. All three composition tests pass when run serially. No performance implementation was changed.

Frontend browser navigation and mesh protocols were not qualified by this task.
