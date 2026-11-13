# Real backend tests

Run from the `of` repository. Keep `ofdb`, `ofnet`, and `offs` beside it. Integration `8596dfa08` also requires the `ofdb-buildfix` and `offs-buildfix` sibling aliases described in `.scratch/backend-simplification/buildfix-validation.md`. CI checks out those exact dependency commits. Worktrees need the same layout. Do not remove task worktree directories.

## Prepare

Use the Node version in `.nvmrc` and pnpm 12.4.1. Password-manager links to `../ofdb/packages/sql`; its compiled JS and WASM must exist. On a clean checkout, build that package with its documented Rust/wasm-pack versions. The CI workflow runs these steps. The checked-in `packages/sql-wasm` is a different, lower-level package; it does not replace the wrapper.

```sh
pnpm --dir ../ofdb install --frozen-lockfile --ignore-scripts
pnpm --dir ../ofdb --filter @ofsdk/sql run build:wasm
pnpm --dir ../ofdb --filter @ofsdk/sql run build:esm
pnpm install --frozen-lockfile --ignore-scripts
pnpm exec playwright install --with-deps chromium
pnpm e2e:prepare
cargo build --locked -p unified-server
```

## Run

```sh
pnpm test:e2e:check
pnpm test:e2e:lifecycle
pnpm test:backend
pnpm test:e2e
```

`pnpm test:backend` runs the app and password-manager Vitest browser suites in sequence. Each suite starts its own real backend. Runs are serial; a failing first suite does not suppress the second, and any failure returns a nonzero exit code. The normal password-manager `pnpm test` still runs offline vault unit tests without a backend.

Each app also has `pnpm test:e2e`. Run it from that app directory to select its Playwright project. The root Playwright run covers both frontends plus owner HTTP contracts. API coverage for both client registrations is in the app project.

## Lifecycle and isolation

`backend.mjs` is the global setup for both runners. Vitest installs it on the Chromium instance only: installing the same setup on both the root and browser project starts two servers. By default it runs `cargo run --locked -p unified-server -- local ... --no-tls` on port 1355, with a fresh OS temporary directory. It waits for all four owner health routes. It reports build/process failures and missing health routes. The default deadline is 180 seconds, including Cargo; set `E2E_START_TIMEOUT_MS` for a slower first build. This is not a three-second startup benchmark.

To use an already-built executable, set `E2E_BACKEND_BIN` to its path, relative to the repository root or absolute. This still starts a new process and new temporary data; it never connects to an existing developer backend. For the shared task worktrees:

```sh
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm test:e2e
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm --filter @ofsdk/app test
E2E_BACKEND_BIN=../buildfix/target/debug/unified-server pnpm --filter @ofsdk/password-manager test:backend
```

Teardown terminates the process group before removing only its temporary backend directory. Failed startup uses the same cleanup. The launcher refuses an occupied port. Do not run backend suites in parallel or reuse a developer backend. An external SIGKILL cannot run JavaScript teardown.

The frontend `.env.test` files and runner environment use HTTP on `127.0.0.1`: app port 5173 and password-manager port 5174. Vitest uses these ports too, so real CORS checks apply. Password-manager also needs `PUBLIC_STORAGE_URL`, which its current sync code reads. These are public test URLs, not secrets. Only generated reports under `.scratch/e2e-results` and `.scratch/e2e-report` are ignored.

## Test boundaries

- Playwright: owner health/discovery, failed sign-in, consent plus PKCE exchanges for both registered clients, Storage SQL CRUD, Device enrollment/approval/listing, app session access, and the password-manager redirect/consent/callback flow.
- Vitest with Chromium: real browser CORS, sign-in/User info, Device access, Storage writes, and encrypted password-manager vault CRUD through the real sync SDK. The vault test reads remote ciphertext through the public Storage API; it cannot pass with a local-only write.
- Node lifecycle tests: a small child-process fixture checks supervision only. It is not an identity, Device, or Storage substitute. No domain test intercepts HTTP or supplies fake owner clients.

Removed app tests supplied fake backend and native-owner responses. Their independent setup, revocation, residency, and failure-recovery cases are not all replaced by the local single-node suite. Independent/desktop qualification is a separate gap, not a claim made by these tests. Local vault tests keep their browser-storage failure injection; those are not backend mocks.

## CI

`.github/workflows/e2e.yml` checks out the sibling repositories plus the pinned qualified Rust dependency aliases, builds the linked SQL wrapper, installs Chromium, builds frontend dependencies and Rust, then runs both real test runners against that compiled binary. Each runner step executes after a successful build even if another runner fails; failures still fail the job. It has read-only repository permission, a bounded job, no stored checkout credentials, and failure artifacts. The pinned Rust dependency commits must be available from their remotes. The workflow does not bypass failures or substitute a fake server.

See [runtime validation](runtime-validation.md) for the latest measured results and [coordinator blockers](backend-blockers.md) for concrete failing requests. The older `.scratch/backend-simplification/e2e-validation.md` records the pre-buildfix run. A test being listed or a frontend build passing does not qualify a real backend flow.
