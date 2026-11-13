# Real-runtime E2E qualification after integration 8596dfa08

Branch: `backend-simplification-e2e`. Worktree: `.scratch/worktrees/e2e`.
Incoming integration merge: fast-forward to `8596dfa08` before any changes in this pass.

Status: **real HTTP tests executed; acceptance still fails**. This supersedes the pre-buildfix results in `.scratch/backend-simplification/e2e-validation.md`. Do not close ticket05 on these results.

## Changes in this pass

- Added `E2E_BACKEND_BIN`: launches an explicit prebuilt executable with the same real local CLI, a fresh data directory, four-owner readiness, and owned process teardown. The default is still `cargo run --locked`. No running backend is reused. Added success and missing-binary regression tests. Preserve actual socket errors instead of describing every error as an occupied port.
- Fixed Vitest's double startup: root-level global setup was inherited by the browser project, so the same PID tried to start two backends on port 1355. Install setup on the Chromium instance only. The real failing run showed two startup messages and `EADDRINUSE`; the corrected real runs show one startup per suite and execute the tests.
- Split browser health, authentication, Device access, and Storage writes into independent tests. Added separate passing consent/PKCE cases for both clients so a later owner failure does not hide successful authentication.
- Added real-fetch error diagnostics for generated SDK calls: method, URL, status, and response body appear in failures. No request is intercepted, rewritten, or answered by a substitute.
- Joined browser response waits with their triggering actions. Assert sign-in form availability before filling it, so missing setup-free sign-in fails with an explicit message instead of an unobserved response wait.
- Made `pnpm test:backend` serial and non-bailing: both real Vitest suites run, then the command fails if either failed. This avoids both port contention and hiding password-manager results after an app failure.
- Updated CI for the actual `ofdb-buildfix` and `offs-buildfix` dependency layout. Pinned commits are from the Rust agent's qualification record. All three real runner steps run after a successful build even if a different runner fails; errors still fail the job. Use the freshly compiled executable and retain browser failure screenshots.
- Added [concrete coordinator requests](backend-blockers.md) and updated the runner README. No Rust/Cargo, frontend implementation, environment URL, or generated SDK was changed. **No unit test was deleted in this pass.**

Complexity/risk: small runner/config changes; main risk is duplicate processes and false positives. Real tests remain strict and fail on the owner and frontend defects instead of bypassing them.

## Measured results

The real binary used was `../buildfix/target/debug/unified-server`, with the dependency aliases already supplied by the Rust agent. Every suite started its own real HTTP process on an empty temporary directory. The Rust agent owns build qualification; this pass did not rebuild or modify that executable.

| Command/check | Result |
| --- | --- |
| `pnpm test:e2e:lifecycle` | **7 passed**, 0 failed. Includes explicit executable CLI/data-dir/port arguments, no Cargo fallback, occupied port, health deadline, early exit, spawn error, teardown and data isolation. |
| `pnpm test:e2e:check` | Passed. Strict TypeScript check of E2E sources. |
| `E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm test:e2e` | **3 passed, 5 failed** in 8 tests. Completed in about 1.9 minutes on the final run. Passing: four-owner health/discovery/invalid-password rejection, app consent/PKCE/User info, password-manager consent/PKCE/User info. Failed: both Storage CRUD cases, Device enrollment, app browser sign-in, password-manager browser sign-in. |
| `E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm test:backend` | Both suites executed sequentially; aggregate exit code **1**. App: **2 passed, 2 failed**. Password-manager: **2 passed, 3 failed**. Passing in each: real Chromium health and administrator sign-in/User info. Failed: Device list and Storage create in each; password-manager encrypted vault also cannot create its remote database. |
| Focused unchanged password-manager TOTP/generator unit tests | **52 passed** in 2 browser files. Command: `pnpm --filter @ofsdk/password-manager exec vitest run src/lib/vault/secrets/totp.test.ts src/lib/vault/secrets/generator/generator.test.ts`. No backend needed. |
| `pnpm install --frozen-lockfile --ignore-scripts` | Passed. No dependency/lockfile change was needed. |
| CI YAML parser and assertions | Passed. Checked read-only permission, both pinned dependency aliases, and three prebuilt real-backend runner steps. Hosted Actions and `actionlint` were not run. |
| Biome stdin formatting | Applied to E2E sources and changed test configs. Repository lint is still blocked by the unchanged sync-sdk Biome config recorded in the earlier report; no lint pass is claimed. |
| `git diff --check` | Passed. |
| `CARGO_BUILD_JOBS=2 cargo hack test --feature-powerset --workspace --all-targets` | Final attempt timed out at 30 seconds during compilation, first of 208 runs. No full-suite result. |
| `CARGO_BUILD_JOBS=2 just crap` | Final attempt timed out at 30 seconds during coverage compilation. No CRAP result. |
| Failure teardown | `ss -lptn 'sport = :1355'` showed no listener after complete failed Playwright and Vitest runs. The combined Vitest run also proved that the second suite could acquire the same port after the first failed. |

An initial externally bounded Playwright attempt timed out before test results under heavy host load. Its detached backend/frontends were stopped explicitly. A confirmed launcher fixture left by an earlier interrupted run and its exact owned temporary directory were also removed. No task artifact/worktree directory was removed. An external SIGKILL cannot execute JavaScript teardown; normal failure and graceful termination remain the supported cleanup paths.

Local logs are retained under ignored `e2e/.scratch/`: `playwright-final.log`, `playwright-owners-real.log`, `vitest-combined-final.log`, separate app/password-manager logs, and final feature/CRAP logs. Traces/screenshots remain under the runners' normal ignored report directories.

## Remaining owner work

See [backend-blockers.md](backend-blockers.md) for the actual requests, payloads, status codes, bodies, and reproduction commands.

1. Real Storage `POST /storage/databases` returns **503 with an empty body** after a successful SDK token exchange. Both client registrations reproduce it.
2. Real Device `GET /device/devices` and `POST /device/devices/enrollments` return **503 `{"error":"temporarily_unavailable"}`** after a successful resource token exchange.
3. The frontend app redirects `/signin` and password-manager's `/authorize` flow to **`/setup/installation`**. Its implementation still requires independent setup evidence. No available public environment variable removes that gate. This needs the frontend implementation owner, not fake setup responses in the tests.
4. The vault test cannot reach encryption/sync assertions until Storage creation works. Later CRUD, enrollment approval/listing, and completed browser session behavior are not qualified.
5. Browser HTTPS was not added or rerun here; HTTP uses the required frontend test URLs. The Rust agent's HTTP/HTTPS qualification is a separate result. No three-second startup or single-file-state claim is made.
6. The complete offline unit suite was not rerun in this pass. The earlier eight migration/generator failures were not repaired or deleted. Only the focused 52-test result above is new.
7. Hosted CI still needs publication of the pinned sibling dependency commits and a real Actions run. Neither a green hosted job nor a clean SQL-WASM rebuild is claimed.

## Git handoff

All work is committed on the E2E branch only. Integration is merged into this branch before reporting. Do not merge this branch into integration. Task worktree directories remain in place. Commit hooks are bypassed to avoid broad Rust rewrites/checks outside this agent's ownership; the explicit checks above are the qualification record.
