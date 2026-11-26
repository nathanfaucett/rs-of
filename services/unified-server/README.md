# Unified development Node

Use the [default Portless workflow](../../README.md#local-backend). Both setup choices require Unix and run on the same local HTTPS origin. The browser and Rust must trust the Portless CA. Unified setup uses the private embedded key store and does not require an OS credential-store prompt. Protect the data root and its backups: the database contains private keys and service credentials.

## Set up and restart

Use the protected Node code from the private file whose path the server prints. Select **Start a new Installation** or **Join an existing Installation**. You cannot change the choice for that root. The browser reuses the code from memory only. It does not persist the code. Reenter it to resume setup after a reload or an OIDC sign-in redirect. Do not share the code or put it in a URL.

- **Create:** Enter the first administrator's username and password. The Node provisions IdP, Management, Device, and Storage in one `unified.redb`. The app waits for server-sent updates without polling. If the connection closes, select **Check Node status**. If Create stops, use the Node code and the original administrator details to retry. Cancel stops pending work but does not roll back changes already committed. Independent and native owner setup are unchanged.
- **Join:** Supply the existing Installation's HTTPS bases, an OAuth Client registered for this app's `/setup/join` redirect, and the accepting Device public key. Sign in to the existing Installation with permission to enroll Devices. Request pairing, then approve this Device in the existing Installation. The Node code does not replace remote approval, grant Resource access, or select Resources. If interrupted, use **Resume join** with the Node code, then retry the exact pairing with the same User. The Device owner must support exact pairing retries. Join uses the shared database for its local state and does not start local owner services.

Keep the data root and HTTPS origins unchanged on restart. `node-admission/state` contains only the Node code. Locks remain on disk, but setup state is in `unified.redb`.

The initial single-database layout could retain the database at `node-admission/unified.redb`. After acquiring the root lock, setup atomically moves that file to the data root. It rejects two retained database files as ambiguous.

There is no in-place migration from the previous split-database layout, other earlier setup formats, or seeded roots. The server rejects them without modification. Preserve the old root and use `just unified --data-dir <fresh-private-path>` for a new Node.

The [Node setup contract](../../openapi/node.json) publishes IdP and Storage bases only after the Node verifies Create or verifies the selected Installation’s owner evidence and records verified Device approval. The local Node permits Password Manager to read the descriptor. After Join, the existing Installation must permit the exact Password Manager origin on its IdP and Storage APIs. Separate IdP and Storage bases can be used for sign-in; the reference app does not offer legacy vault replication with those bases.

## Explicit seeded fixture

Run `just unified-seeded` and `pnpm apps:dev:seeded` to use the seeded fixture with Portless. Its default data root is `.tmp/unified-seeded`; to open a preserved old seeded root, pass `--data-dir` with that path. The direct command `cargo run --locked -p unified-server -- seeded` serves loopback HTTP. Its seeded `admin@local.test` / `password` login is for tests only. Do not expose it to an untrusted network.

## Browser qualification

The [first-run Chromium tests](../../e2e/create-installation.e2e.ts) run the actual launch recipes behind a private Portless HTTPS proxy. See [E2E setup](../../e2e/README.md) for build and browser prerequisites. The tests need `just`, `openssl`, `certutil`, and loopback resolution for Portless hostnames. CI runs the separate loopback HTTPS [Join fault test](../../e2e/join.md); it does not run the full Portless suite.

```sh
cargo build --locked -p unified-server
pnpm exec playwright test --config e2e/create.playwright.config.ts
```
