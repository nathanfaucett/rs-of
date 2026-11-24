# of — Offline-first applications

`of` builds local-first applications with OAuth 2.0 / OpenID Connect identity, user- and application-owned data, shared synchronization Devices, and durable offline storage.

The reference workflow is an encrypted password vault that works on one Device without an account and can later synchronize through a user-controlled Sync Server. See the [product goal and north-star workflow](docs/goal.md).

## Current status

The repository is migrating to the [accepted target architecture](docs/architecture.md). Target design is not a claim of completed implementation.

| Area                                   | Current state                                                                                                                                                                   |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Local unified backend                  | Available for loopback development; mounts IdP, Management, Device, and Storage APIs                                                                                            |
| OIDC and remote Storage SQL            | Discovery, consent/PKCE sign-in, session reload, and remote Storage SQL CRUD pass real-backend Playwright tests                                                                 |
| Local encrypted password vault         | Development reference application, not a production password manager                                                                                                            |
| Shared multi-User Installation Devices | Accepted target; migration is incomplete                                                                                                                                        |
| Two-Device offline convergence         | Not qualified                                                                                                                                                                   |
| Browser synchronization                | Single-Node encrypted-vault sync passes real-backend Chromium tests; broader browser support remains incomplete                                                                 |
| Linux FUSE mounts                      | Full Residency only; local file/directory operations and one authorized native two-Full sync/reopen scenario pass real mounts; broader multi-Device support remains unqualified |
| Independent and desktop deployments    | Can create a first Installation. An existing Installation admits Devices, not additional server authorities.                                                                    |

GitHub Actions records current and historical check results.

Installation setup requires fresh owner stores. If you have an older setup, back up its data and redo setup in an empty root. This change provides no migration, old-format reader, or version bump.

To add a Device to an existing Installation, use the [Device owner pairing contract](openapi/device.json). An authorized User creates a pending request against an approved Device. The new Device signs the UTF-8 payload with its Ed25519 endpoint key and encodes the signature as base64url without padding. An authorized User submits that signature to approve the Device. No server setup or transport listener is required.

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

Each owner has a `/health` route. In a second terminal, run `pnpm apps:dev` to start both frontends. Their default Portless URLs use the local backend without environment variables. Open [Password Manager](https://password-manager.localhost:1355) or the [main app](https://of.localhost:1355).

Local mode seeds test identities, including `admin@local.test` / `password`. Do not expose these credentials or the local listener to an untrusted network.

Direct `cargo run -p unified-server -- local` serves plain HTTP on a fixed loopback port. TLS belongs at Portless, nginx, or a load balancer. Local signer state is bound to the public issuer URL; use a fresh data directory when that URL changes. This mode is not production deployment.

## Repository map

| Path        | Contents                                                                          |
| ----------- | --------------------------------------------------------------------------------- |
| `services/` | IdP, Management, Device, Storage, shared models, servers, and generated clients   |
| `db/`       | `ofdb-sql` and `ofdb-kv`, their engines, persistence adapters, and sync protocols |
| `fs/`       | Offline-first filesystem                                                          |
| `net/`      | Iroh endpoint, pairing, mesh, and byte-stream support                             |
| `packages/` | TypeScript clients and browser wrappers                                           |
| `apps/`     | Main application and the small password-manager reference application             |
| `openapi/`  | Generated HTTP contracts                                                          |
| `e2e/`      | Real-backend browser tests and lifecycle fixtures                                 |

This checkout is canonical. Do not copy changes to sibling `ofdb`, `ofnet`, or `offs` repositories.

## Development

```bash
# Rust
just build
just test
just clippy
just crap

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

- [Glossary map](GLOSSARY-MAP.md)
- [Contributing](CONTRIBUTING.md)
- [Real-backend tests](e2e/README.md)

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
