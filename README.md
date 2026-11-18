# of — Offline-first applications

`of` builds local-first applications with OAuth 2.0 / OpenID Connect identity, user- and application-owned data, shared synchronization Devices, and durable offline storage.

The reference workflow is an encrypted password vault that works on one Device without an account and can later synchronize through a user-controlled Sync Server. See the [product goal and north-star workflow](docs/goal.md).

## Current status

The repository is migrating to the [accepted target architecture](docs/architecture.md). Target design is not a claim of completed implementation.

| Area | Current state |
| --- | --- |
| Local unified backend | Available for loopback development; mounts IdP, Management, Device, and Storage APIs |
| OIDC and remote Storage SQL | Real-backend tests exist; current discovery/sign-in qualification is not established |
| Local encrypted password vault | Development reference application, not a production password manager |
| Shared multi-User Installation Devices | Accepted target; migration is incomplete |
| Two-Device offline convergence | Not qualified |
| Browser synchronization | Incomplete; do not infer support from transport packages alone |
| Independent and desktop deployments | Retain implementation paths, but current end-to-end qualification is not established |

Historical [E2E validation](e2e/runtime-validation.md) is commit-specific evidence, not a current release guarantee. CI is the source of current check status.

## Local backend

Prerequisites are defined by `rust-toolchain.toml`, `Cargo.toml`, and `.nvmrc`. Install `just` and pnpm 12.4.1 for the full development workflow.

```bash
just unified
```

Portless terminates HTTPS and assigns a dynamic internal port. The server binds to loopback and exposes:

- `https://unified.localhost:1355/idp`
- `https://unified.localhost:1355/management`
- `https://unified.localhost:1355/device`
- `https://unified.localhost:1355/storage`

Each owner has a `/health` route. Local mode seeds test identities, including `admin@local.test` / `password`. Do not expose these credentials or the local listener to an untrusted network.

Direct `cargo run -p unified-server -- local` serves plain HTTP on a fixed loopback port. TLS belongs at Portless, nginx, or a load balancer. Local signer state is bound to the public issuer URL; use a fresh data directory when that URL changes. This mode is not production deployment.

## Repository map

| Path | Contents |
| --- | --- |
| `services/` | IdP, Management, Device, Storage, shared models, servers, and generated clients |
| `db/` | `ofdb-sql` and `ofdb-kv`, their engines, persistence adapters, and sync protocols |
| `fs/` | Offline-first filesystem |
| `net/` | Iroh endpoint, pairing, mesh, and byte-stream support |
| `packages/` | TypeScript clients and browser wrappers |
| `apps/` | Main application and the small password-manager reference application |
| `openapi/` | Generated HTTP contracts |
| `e2e/` | Real-backend browser tests and commit-specific evidence |

This checkout is canonical. Do not copy changes to sibling `ofdb`, `ofnet`, or `offs` repositories.

## Development

```bash
# Rust
just build
just test
just clippy
just crap

# Documentation
just docs-check

# TypeScript dependencies
pnpm install --frozen-lockfile --ignore-scripts

# Real-backend browser test preparation and tests
pnpm e2e:prepare
pnpm test:e2e
```

Full browser preparation builds local WASM artifacts and generated clients. It needs `wasm-pack`, the `wasm32-unknown-unknown` Rust target, and browser dependencies. See [E2E setup](e2e/README.md) before running the complete suite.

Use change-appropriate focused checks during development. See [Contributing](CONTRIBUTING.md).

## Documentation

- [Goal, north-star workflow, and milestones](docs/goal.md)
- [Target architecture and ownership](docs/architecture.md)
- [Deployment modes and limits](docs/backend-deployments.md)
- [Browser storage and packages](docs/browser.md)
- [Glossary map](GLOSSARY-MAP.md)
- [Contributing](CONTRIBUTING.md)
- [Real-backend tests](e2e/README.md)

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
