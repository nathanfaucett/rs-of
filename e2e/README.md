# Real backend tests

Run from the root of a single `nathanfaucett/rs-of` checkout. Do not check out or build sibling `rs-ofdb`, `rs-ofnet`, or `rs-offs` repositories. All required Rust, WASM, and TypeScript sources are in this repository.

## Prepare

Use the Node version in `.nvmrc`, pnpm 12.4.1, Rust 1.100 with the `wasm32-unknown-unknown` target, wasm-pack 0.13.1, and the Protocol Buffers compiler (`protoc`). The backend also needs `pkg-config` and the D-Bus development package. On Debian or Ubuntu, install the native packages with `sudo apt-get update && sudo apt-get install -y --no-install-recommends pkg-config libdbus-1-dev protobuf-compiler`.

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

## Default first-run setup

Run `pnpm exec playwright test --config e2e/create.playwright.config.ts` to test both choices and retained state through the default launchers, real Portless HTTPS, and the real Password Manager descriptor. See [unified-server](../services/unified-server/README.md#browser-qualification) for additional prerequisites.

## Lifecycle and isolation

`backend.mjs` is the global setup for both runners. Vitest installs it on the Chromium instance only: installing the same setup on both the root and browser project starts two servers. It explicitly runs the seeded fixture with `cargo run --locked -p unified-server -- seeded ...` as plain HTTP on an OS-assigned loopback port, with a fresh OS temporary directory. Independent `startBackend` calls also allocate a port when `port` is omitted. It waits for all four owner health routes. It reports build/process failures and missing health routes. The default deadline is 180 seconds, including Cargo; set `E2E_START_TIMEOUT_MS` for a slower first build. This is not a three-second startup benchmark. The launcher sets `RUST_MIN_STACK=67108864` by default because the current backend startup overflows a smaller stack; set the variable explicitly to override that test environment.

To use an already-built executable, set `E2E_BACKEND_BIN` to its path, relative to the repository root or absolute. This still starts a new process and new temporary data; it never connects to an existing developer backend. For the repository-local build above:

```sh
E2E_BACKEND_BIN=./target/debug/unified-server pnpm test:e2e
E2E_BACKEND_BIN=./target/debug/unified-server pnpm --filter @ofsdk/app test:backend
E2E_BACKEND_BIN=./target/debug/unified-server pnpm --filter @ofsdk/password-manager test:backend
```

Teardown terminates the process group before removing only its temporary backend directory. Failed startup uses the same cleanup. The launcher refuses an explicitly requested occupied port. Do not run backend suites in parallel or reuse a developer backend. An external SIGKILL cannot run JavaScript teardown.

The runner reserves distinct OS-assigned app and password-manager ports and shares them with Playwright workers through `E2E_BACKEND_PORT`, `E2E_APP_PORT`, and `E2E_PASSWORD_MANAGER_PORT`. Playwright starts each Vite server on its assigned port with `--strictPort`. Vitest reads the same dynamic backend URL through `VITE_E2E_BACKEND_URL` and takes its browser API ports from the shared frontend URLs. Password-manager also needs `PUBLIC_STORAGE_URL`, which its current sync code reads. These are public test URLs, not secrets.

Set any of the three port variables to a free fixed port for debugging. Without an explicit password-manager origin, unified-server retains its port 5174 default. Only generated reports under `.tmp/e2e-results` and `.tmp/e2e-report` are ignored.

## Browser configuration and desktop isolation

The app permits setup-free browser login only when `PUBLIC_IDP_BASE_URL` and `PUBLIC_OIDC_CLIENT_ID` are configured and the selected IdP matches. The default Portless workflow uses verified Node setup instead. The seeded dev launch and seeded test runner supply explicit fixture Clients. Live discovery must confirm the exact issuer and token endpoint. Only HTTPS or HTTP loopback is accepted. Tauri never takes this browser path; independent desktop setup remains separate. Without a configured browser client, the setup gate remains.

`appEnv` uses `local-login`; password-manager uses `password-manager`. Configure `PUBLIC_OIDC_APPLICATION_URI` to the expected Application identity, not the redirect origin. The runner derives these identities from `appURL`, and the local seed uses both runner origins for client redirect URIs. These are runner settings, not production defaults. Password-manager fails closed when this identity is missing. `PUBLIC_OIDC_SCOPES` overrides the existing default; the local seed accepts `openid profile email storage`.

Password-manager uses the SDK's existing local storage mode so the PKCE verifier survives external navigation and the session survives reload. Access/refresh tokens therefore persist in browser local storage and are accessible to same-origin script. Logout uses the SDK's existing token clearing. No session-storage mode was added.

## Test boundaries

The fresh seeded fixture grants `idp.clients.read` to the test administrator only in the `local-login` Application. Registration by itself grants no read access. The tests confirm IdP lookup succeeds with the bound Application token and fails with a different Application token for the same User. The lookup retry test interrupts one browser request, then retries against the real IdP. This fixture does not change Installation setup policy or backfill existing data roots.

- Playwright: owner health/discovery, failed sign-in, consent plus PKCE exchanges for both registered clients, Storage SQL CRUD, first-Device enrollment and signed second-Device approval through the Device owner, app session access, and the password-manager sign-in link, redirect, consent, callback, and session-reload flow.
- Account security: Playwright uses a synthetic target to check Block confirmation, cancellation, IdP denials, and interrupted MFA requests. The unified owner composition test grants only the exact Installation permissions to check successful Block, Unblock, and TOTP changes.
- Password reset uses synthetic Users. Chromium checks the missing-destination gate, Cancel and focus return, and recovery from an injected HTTP 500. It also changes a password against the real IdP and checks that only the replacement works. It does not test email delivery.
- Management layout checks use the seeded Application and User records and the real Device list. They check form access and record reflow at 320, 390, 768, and 1440 CSS pixels, plus keyboard navigation and retained preferences. They do not save form drafts.
- Vitest with Chromium: real browser CORS, sign-in/User info, Device access, Storage writes, and encrypted password-manager vault CRUD through the real sync SDK. The vault test reads remote ciphertext through the public Storage API; it cannot pass with a local-only write.
- Node lifecycle tests: a small child-process fixture checks supervision only. It is not an identity, Device, or Storage substitute. Application administration uses the real IdP for create, edit, delete, and pagination. The pagination test aborts one browser request to verify recovery; it does not supply a fake owner response.

Restored isolated native capability tests do not mock backend HTTP. Registration trust-boundary logic is tested separately. Essential authorization-page assertions now run through real browser login and server consent metadata, including deny/state behavior and malformed requests. Removed backend-mocked independent setup, revocation, residency, and failure-recovery cases are not all replaced by the local single-node suite. Independent/desktop qualification is a separate gap, not a claim made by these tests. Local vault tests keep their browser-storage failure injection; those are not backend mocks.

## CI

The `Real backend E2E` job in `.github/workflows/ci.yml` runs only after the documentation, Cargo, and OpenAPI jobs pass. The workflow builds a content-addressed GHCR image with cached Docker layers so the Rust and Node jobs share their installed tools. The E2E job checks the Rust toolchain, WASM target, wasm-pack, and `protoc` before restoring cached Cargo, pnpm, and Chromium data, builds the local WASM and TypeScript dependencies and Rust backend, then runs both real test runners against that compiled binary. Each test-runner step executes after a successful build even if another test runner fails; failures still fail the job. It has read-only repository permission, a bounded job, concurrency cancellation, no stored checkout credentials, and failure artifacts. The workflow does not bypass failures or substitute a fake server.

GitHub Actions records current and historical results. A test being listed or a frontend build passing does not qualify a real backend flow.
