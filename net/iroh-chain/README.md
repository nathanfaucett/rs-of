# `iroh-chain`

Reusable Iroh endpoint, pairing, allowlist, routing, mesh-message, and byte-stream crate for the `of` ecosystem.

A native Node may share one endpoint/router across service contexts. `of` persists Installation/Device trust and supplies protocol-specific authorization. `iroh-chain` does not interpret User, Application, Resource, or Selection policy.

Use Rustdoc and the crate's public integration tests for API details. `packages/net` contains the private browser wrapper.

```bash
cargo test -p iroh-chain --all-targets
pnpm --filter @ofsdk/net run build
```

See [networking and synchronization](../../docs/architecture.md#networking-and-synchronization) for ownership boundaries. Endpoint pairing and allowlisting must not be used as substitutes for Device approval or Resource authorization.
