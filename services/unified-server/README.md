# Unified first-run setup

On Unix, opt in to creating a new Installation through local Portless HTTPS:

```sh
pnpm exec portless run --name unified cargo run --locked -p unified-server --features independent-contracts -- setup
```

In another terminal, run `pnpm apps:dev` and open `https://of.localhost:1355`.
If Portless uses another app origin (for example, in a Git worktree), pass that exact HTTPS origin to `setup --origin`.
Use the codes from the three protected local files printed by the server. Do not share the codes or put them in URLs.

Portless must have a trusted certificate. The browser and Rust must both trust its CA. Use the same unlocked native credential store across restarts. On Linux the normal provider is the desktop Secret Service; it may ask to create or unlock the `IdP` collection. Setup does not fall back to a file or disable TLS verification.

The default root is `.data/setup`, separate from the seeded owner stores. To use another root, pass `--data-dir` with a new private directory outside version control or the exact previously selected setup root. Do not point setup at a seeded `.data` root. Keep the public URLs and retained root unchanged when restarting. If Create was interrupted, recover the exact operation, then retry with the active owner codes and original IdP password.

`just unified` still runs the seeded development fixture. This opt-in does not join another Installation or register a Password Manager OAuth Client.

The [Create browser test](../../e2e/create-installation.e2e.ts) uses a private Portless proxy and explicit CA trust. On Linux it requires Chromium, `openssl`, `certutil`, built app dependencies, and an unlocked native Secret Service with the `IdP` collection. Build and run the test:

```sh
cargo build --locked -p unified-server --features independent-contracts
pnpm exec playwright test --config e2e/create.playwright.config.ts
```
