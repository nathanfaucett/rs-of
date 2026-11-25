# Unified development Node

Use the [default Portless workflow](../../README.md#local-backend). Both setup choices require Unix and run on the same local HTTPS origin. The browser and Rust must trust the Portless CA. Create also needs the normal OS credential store. On Linux, unlock the desktop Secret Service; it can ask you to create or unlock the `IdP` collection. There is no fallback to a file or an unverified TLS connection.

## Set up and restart

Use the Node setup code from the private file printed in the server terminal to select one option. The choice cannot be changed for that root.

- To create an Installation, read the three owner setup codes from the private files printed after you select Create. Enter a new administrator username and password. Create publishes each fresh owner host only after it has initialized private admission. An interruption before publication can restart without resetting any published owner state. If Create stops after publication, recover the exact operation and retry with the active owner codes and original IdP password. An older, incomplete published host requires explicit recovery.
- To join an Installation, read the Join setup code from the private file printed after you select Join. Supply the existing Installation's HTTPS bases, an OAuth Client registered for this app's `/setup/join` redirect, and the accepting Device public key. Sign in to the existing Installation with permission to enroll Devices. Request pairing, then approve this Device. This does not grant Resource access or select any Resources. If interrupted, use **Resume join** with the same Join code, then retry the exact pairing with the same User. The Device owner must support exact pairing retries. No helper process is needed.

Do not share setup codes or place them in URLs. Keep the data root, HTTPS origins, and native credential store unchanged on restart. If the retained data came from the seeded fixture or from the old opt-in Create runtime, preserve it and select a fresh root with `just unified --data-dir <fresh-private-path>`. The new runtime does not adopt or change those roots. For Create, use a short absolute path so the IdP Unix socket fits the OS path limit.

The [Node setup contract](../../openapi/node.json) publishes IdP and Storage bases only after the Node verifies Create or verifies the selected Installation’s owner evidence and records verified Device approval. The local Node permits Password Manager to read the descriptor. After Join, the existing Installation must permit the exact Password Manager origin on its IdP and Storage APIs. Separate IdP and Storage bases can be used for sign-in; the reference app does not offer legacy vault replication with those bases.

## Explicit seeded fixture

Run `just unified-seeded` and `pnpm apps:dev:seeded` to use the seeded fixture with Portless. Its default data root is `.tmp/unified-seeded`; to open a preserved old seeded root, pass `--data-dir` with that path. The direct command `cargo run --locked -p unified-server -- seeded` serves loopback HTTP. Its seeded `admin@local.test` / `password` login is for tests only. Do not expose it to an untrusted network.

## Browser qualification

The [first-run Chromium tests](../../e2e/create-installation.e2e.ts) run the actual launch recipes behind a private Portless HTTPS proxy. See [E2E setup](../../e2e/README.md) for build and browser prerequisites. The tests need `just`, `openssl`, `certutil`, loopback resolution for Portless hostnames, and an unlocked native credential store for Create. CI runs the separate loopback HTTPS [Join fault test](../../e2e/join.md); it does not run the full Portless suite.

```sh
cargo build --locked -p unified-server
pnpm exec playwright test --config e2e/create.playwright.config.ts
```
