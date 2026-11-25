# Opt-in Device join

The join helper serves only Device join on loopback HTTPS. It does not start an IdP, create a User, bind an Iroh listener, or make a Selection. The [default `just unified` flow](../README.md#local-backend) runs Join directly and does not need this helper.

## Run manually

```sh
cargo build -p setup --features join --bin join-node
./target/debug/join-node init ./join-node-data 4355 https://of.localhost:1355
./target/debug/join-node serve ./join-node-data 4355 https://of.localhost:1355
```

Use a fresh root for `init`. Verify and trust the helper's `tls/ca.pem` in the browser before opening `/setup/join` on the configured app origin. Read the setup code from the owner-only `admission/state` file. Do not paste the code into a URL or share the root. The helper retains the Ed25519 key and join record in an owner-only file. Do not copy this file to another Node. This slice supports Unix owner-only storage; it does not install a trust anchor or use an OS keyring.

Select the Installation's canonical HTTPS IdP, Device and Storage bases. The Installation must already have an approved accepting Device and a public OAuth client for the exact join callback (`https://of.localhost:1355/setup/join` in the example). The client must allow authorization code with S256 PKCE and token exchange, with `openid profile email` scopes. The IdP must allow requests from the join app origin and have a working sign-in and consent UI. Sign in as an active User who can read and enroll Devices and matches the accepting Device's current legacy owner binding.

After an interruption, use **Resume join** with the same helper and setup code. **Request pairing** retries the exact retained enrollment with the same key. The Device owner must support [exact pairing retries](../openapi/device.json). The authenticated requester must still have enrollment permission. The helper does not fall back to a new legacy request. The helper retires admission after verifying Installation evidence and recording approval. Later status displays retained identity, not live revocation state.

## Real-backend browser check

Build the app's workspace dependencies and install Chromium as described in [E2E setup](README.md). On Linux, install OpenSSL and `certutil` (the NSS tools package). Then run:

```sh
cargo build -p setup --features join --bin join-node
cargo build -p unified-server --bin unified-server
node e2e/join-fixture.mjs
```

The fixture runs the existing Installation backend and the main app, registers a public join client, and approves the first Device via the fixture's enrollment API. It uses an isolated browser trust store; TLS checks stay enabled. It verifies that Join does not follow a redirected Device request. It drops one request before delivery and one response after the Device owner commits, restarting the helper after each interruption. It verifies that only one Device is enrolled. It checks local admission, real remote sign-in and consent, pending state, wrong proof rejection, approval, and the exact approved identity after helper restart. It does not qualify transport or Resource synchronization.
