# Real backend tests

Run from the root of a single `nathanfaucett/rs-of` checkout. Do not check out or build sibling `rs-ofdb`, `rs-ofnet`, or `rs-offs` repositories. All required Rust, WASM, and TypeScript sources are in this repository.

## Prepare

Use the Node version in `.nvmrc`, pnpm 12.4.1, Rust 1.100 with the `wasm32-unknown-unknown` target, and wasm-pack 0.13.1. The backend also needs `pkg-config` and the D-Bus development package. On Debian or Ubuntu, install the native packages with `sudo apt-get update && sudo apt-get install -y --no-install-recommends pkg-config libdbus-1-dev`.

```sh
rustup toolchain install 1.100 --profile minimal --target wasm32-unknown-unknown
cargo install wasm-pack --version 0.13.1 --locked
export RUSTUP_TOOLCHAIN=1.100
pnpm install --frozen-lockfile --ignore-scripts
pnpm exec playwright install --with-deps chromium
pnpm e2e:prepare
cargo build --locked -p unified-server
```

`pnpm e2e:prepare` builds the local `@ofsdk/sql` WASM, ESM, and browser outputs and the local `@ofsdk/file-system` WASM and TypeScript outputs before building the other required SDKs. Password-manager uses SQL directly and through `sync-sdk`; `sync-sdk` declares the file-system dependency. Neither application nor `sync-sdk` depends on `@ofsdk/net`, so e2e preparation does not build it.

## Run

```sh
pnpm test:e2e:check
pnpm test:e2e:lifecycle
pnpm test:backend
pnpm test:e2e
```

`pnpm --filter @ofsdk/app test` runs isolated browser-configuration, native capability and registration tests without backend HTTP mocks. `pnpm test:backend` runs the app and password-manager Vitest browser suites in sequence. Each suite starts its own real backend. Runs are serial; a failing first suite does not suppress the second, and any failure returns a nonzero exit code. Runner environments use Chokidar polling to avoid shared-host inotify limits; this does not change production watchers. The normal password-manager `pnpm test` still runs offline vault unit tests without a backend.

Each app also has `pnpm test:e2e`. Run it from that app directory to select its Playwright project. The root Playwright run covers both frontends plus owner HTTP contracts. API coverage for both client registrations is in the app project.

## Lifecycle and isolation

`backend.mjs` is the global setup for both runners. Vitest installs it on the Chromium instance only: installing the same setup on both the root and browser project starts two servers. By default it runs `cargo run --locked -p unified-server -- local ...` as plain HTTP on port 1355, with a fresh OS temporary directory. It waits for all four owner health routes. It reports build/process failures and missing health routes. The default deadline is 180 seconds, including Cargo; set `E2E_START_TIMEOUT_MS` for a slower first build. This is not a three-second startup benchmark. The launcher sets `RUST_MIN_STACK=67108864` by default because the current backend startup overflows a smaller stack; set the variable explicitly to override that test environment.

To use an already-built executable, set `E2E_BACKEND_BIN` to its path, relative to the repository root or absolute. This still starts a new process and new temporary data; it never connects to an existing developer backend. For the repository-local build above:

```sh
E2E_BACKEND_BIN=./target/debug/unified-server pnpm test:e2e
E2E_BACKEND_BIN=./target/debug/unified-server pnpm --filter @ofsdk/app test:backend
E2E_BACKEND_BIN=./target/debug/unified-server pnpm --filter @ofsdk/password-manager test:backend
```

Teardown terminates the process group before removing only its temporary backend directory. Failed startup uses the same cleanup. The launcher refuses an occupied port. Do not run backend suites in parallel or reuse a developer backend. An external SIGKILL cannot run JavaScript teardown.

The runner environment in `frontend-env.mjs` uses HTTP on `127.0.0.1`: app port 5173 and password-manager port 5174. Vitest uses these ports too, so real CORS checks apply. Password-manager also needs `PUBLIC_STORAGE_URL`, which its current sync code reads. These are public test URLs, not secrets. Only generated reports under `.scratch/e2e-results` and `.scratch/e2e-report` are ignored.

## Browser configuration and desktop isolation

The app permits setup-free browser login only when `PUBLIC_IDP_BASE_URL` and `PUBLIC_OIDC_CLIENT_ID` are explicitly set and the selected IdP matches. Live discovery must confirm the exact issuer and token endpoint. Only HTTPS or HTTP loopback is accepted. No seeded host or client is inferred. Tauri never takes this browser path; independent desktop setup remains separate. Without an explicit browser client, the existing setup gate remains.

`appEnv` uses `local-login`; password-manager uses `password-manager`. Configure `PUBLIC_OIDC_APPLICATION_URI` to the expected Application identity, not the redirect origin. The local seed uses `http://127.0.0.1:5173/local-login` and `http://127.0.0.1:5173/password-manager`. These are runner settings, not production defaults. Password-manager fails closed when this identity is missing. `PUBLIC_OIDC_SCOPES` overrides the existing default; the local seed accepts `openid profile email storage`.

Password-manager uses the SDK's existing local storage mode so the PKCE verifier survives external navigation and the session survives reload. Access/refresh tokens therefore persist in browser local storage and are accessible to same-origin script. Logout uses the SDK's existing token clearing. No session-storage mode was added.

## Test boundaries

- Playwright: owner health/discovery, failed sign-in, consent plus PKCE exchanges for both registered clients, Storage SQL CRUD, first-Device enrollment/autoapproval/listing, app session access, and the password-manager redirect/consent/callback flow.
- Vitest with Chromium: real browser CORS, sign-in/User info, Device access, Storage writes, and encrypted password-manager vault CRUD through the real sync SDK. The vault test reads remote ciphertext through the public Storage API; it cannot pass with a local-only write.
- Node lifecycle tests: a small child-process fixture checks supervision only. It is not an identity, Device, or Storage substitute. No domain test intercepts HTTP or supplies fake owner clients.

Restored isolated native capability tests do not mock backend HTTP. Registration trust-boundary logic is tested separately. Essential authorization-page assertions now run through real browser login and server consent metadata, including deny/state behavior and malformed requests. Removed backend-mocked independent setup, revocation, residency, and failure-recovery cases are not all replaced by the local single-node suite. Independent/desktop qualification is a separate gap, not a claim made by these tests. Local vault tests keep their browser-storage failure injection; those are not backend mocks.

## CI

`.github/workflows/e2e.yml` checks out only `nathanfaucett/rs-of`, installs Chromium, builds the local WASM and TypeScript dependencies and Rust backend, then runs both real test runners against that compiled binary. Each runner step executes after a successful build even if another runner fails; failures still fail the job. It has read-only repository permission, a bounded job, concurrency cancellation, no stored checkout credentials, and failure artifacts. The workflow does not bypass failures or substitute a fake server.

See [runtime validation](runtime-validation.md) for recorded commit-specific results and [coordinator blockers](backend-blockers.md) for the corresponding failing requests. Historical task notes are not current qualification evidence. A test being listed or a frontend build passing does not qualify a real backend flow.
