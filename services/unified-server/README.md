# Unified development Node

Use the [default Portless workflow](../../README.md#local-backend). Both setup choices require Unix and run on the same local HTTPS origin. The browser and Rust must trust the Portless CA. Create also needs the normal OS credential store. On Linux, unlock the desktop Secret Service; it can ask you to create or unlock the `IdP` collection. There is no fallback to a file or an unverified TLS connection.

## Set up and restart

Use the protected Node code from the private file whose path the server prints. Select **Start a new Installation** or **Join an existing Installation**. You cannot change the choice for that root. The browser reuses the code from memory only. It does not persist the code. Reenter it to resume setup after a reload or an OIDC sign-in redirect. Do not share the code or put it in a URL.

- **Create:** Enter the first administrator's username and password. The Node authorizes Create with the Node code and drives the IdP, Device, and Storage setup workers in the same process. Unified Create does not expose public owner setup routes. Independent and native owner setup are unchanged. If Create stops, use the Node code to recover and retry the exact operation. The original administrator password may be required until the IdP is ready. Exact retry and cancel do not reset data or roll back work already done.
- **Join:** Supply the existing Installation's HTTPS bases, an OAuth Client registered for this app's `/setup/join` redirect, and the accepting Device public key. Sign in to the existing Installation with permission to enroll Devices. Request pairing, then approve this Device in the existing Installation. The Node authorizes Join with the same Node code and validates admission internally against the retained Join code. The Node code does not replace remote approval, grant Resource access, or select any Resources. If interrupted, use **Resume join** with the Node code, then retry the exact pairing with the same User. The Device owner must support exact pairing retries. No helper process is needed.

Keep the data root, HTTPS origins, and native credential store unchanged on restart. Existing active and completed unified roots keep their data and original distinct codes. No migration or code replacement is needed. The operator uses only the Node code.

Seeded roots and roots from the earlier opt-in Create runtime are not supported by this workflow. Preserve them and use `just unified --data-dir <fresh-private-path>` to start fresh. For Create, use a short absolute path so the IdP Unix socket fits the OS path limit.

The [Node setup contract](../../openapi/node.json) publishes IdP and Storage bases only after the Node verifies Create or verifies the selected Installation’s owner evidence and records verified Device approval. The local Node permits Password Manager to read the descriptor. After Join, the existing Installation must permit the exact Password Manager origin on its IdP and Storage APIs. Separate IdP and Storage bases can be used for sign-in; the reference app does not offer legacy vault replication with those bases.

## Explicit seeded fixture

Run `just unified-seeded` and `pnpm apps:dev:seeded` to use the seeded fixture with Portless. Its default data root is `.tmp/unified-seeded`; to open a preserved old seeded root, pass `--data-dir` with that path. The direct command `cargo run --locked -p unified-server -- seeded` serves loopback HTTP. Its seeded `admin@local.test` / `password` login is for tests only. Do not expose it to an untrusted network.

## Browser qualification

The [first-run Chromium tests](../../e2e/create-installation.e2e.ts) run the actual launch recipes behind a private Portless HTTPS proxy. See [E2E setup](../../e2e/README.md) for build and browser prerequisites. The tests need `just`, `openssl`, `certutil`, loopback resolution for Portless hostnames, and an unlocked native credential store for Create. CI runs the separate loopback HTTPS [Join fault test](../../e2e/join.md); it does not run the full Portless suite.

```sh
cargo build --locked -p unified-server
pnpm exec playwright test --config e2e/create.playwright.config.ts
```
