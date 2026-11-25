# of — Offline-first applications

`of` builds local-first applications with OAuth 2.0 / OpenID Connect identity, user- and application-owned data, shared synchronization Devices, and durable offline storage.

The reference workflow is an encrypted password vault that works on one Device without an account and can later synchronize through a user-controlled Sync Server. See the [product goal and north-star workflow](docs/goal.md).

## Current status

The repository is migrating to the [accepted target architecture](docs/architecture.md). Target design is not a claim of completed implementation.

| Area                                   | Current state                                                                                                                                                                   |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Local unified backend                  | First-run setup over Portless HTTPS: create an Installation or join one as a Device                                                                                            |
| OIDC and remote Storage SQL            | Discovery, consent/PKCE sign-in, session reload, and remote Storage SQL CRUD pass real-backend Playwright tests                                                                 |
| Local encrypted password vault         | Development reference application, not a production password manager                                                                                                            |
| Shared multi-User Installation Devices | Accepted target; migration is incomplete                                                                                                                                        |
| Two-Device offline convergence         | Not qualified                                                                                                                                                                   |
| Browser synchronization                | Single-Node encrypted-vault sync passes real-backend Chromium tests; broader browser support remains incomplete                                                                 |
| Linux FUSE mounts                      | Full Residency only; local file/directory operations and one authorized native two-Full sync/reopen scenario pass real mounts; broader multi-Device support remains unqualified |
| Independent and desktop deployments    | Can create a first Installation. An existing Installation admits Devices, not additional server authorities.                                                                    |

GitHub Actions records current and historical check results.

Installation setup requires fresh owner stores. If you have an older setup, back up its data and redo setup in an empty root. This change provides no migration, old-format reader, or version bump.

To add a Device to an existing Installation, use the [Device owner pairing contract](openapi/device.json). An authorized User creates a pending request against an approved Device. Use the exact retry operation when delivery is uncertain: it matches the original requester, endpoint key, and enrollment fields in the same Installation. The new Device signs the UTF-8 payload with its Ed25519 endpoint key and encodes the signature as base64url without padding. An authorized User submits that signature to approve the Device. No server setup or transport listener is required.

## Local backend

Prerequisites are defined by `rust-toolchain.toml`, `Cargo.toml`, and `.nvmrc`. Install `just` and pnpm 12.4.1 for the full development workflow.

First-run setup requires Unix. If Portless is not running, start the trusted HTTPS proxy on the development port:

```bash
pnpm exec portless proxy start --port 1355
```

Start the backend and the two apps in separate terminals:

```bash
just unified
pnpm apps:dev
```

Open the [main app](https://of.localhost:1355). On a fresh Node, enter the protected setup code from the file printed by the server and choose **Start a new Installation** or **Join an existing Installation**. Portless can prefix these hostnames in a Git worktree; use the URLs it prints. You do not need to set environment variables.

- **Create** sets the first administrator and starts the local owner APIs. It does not seed any accounts. Wait for setup to verify readiness before signing in.
- **Join** uses an existing Installation's Device pairing and approval. It does not start local owner APIs or create accounts.

The [Password Manager](https://password-manager.localhost:1355) can use local vaults immediately. For sign-in it reads the selected IdP and Storage from the [Node setup descriptor](openapi/node.json) after setup completes.

Restart with the same data root and public URLs to continue or use the retained setup. The default root is `.data`. The server rejects seeded roots without changing them. Preserve an old root and run `just unified --data-dir <fresh-private-path>` to start setup. There is no migration from seeded state or the earlier opt-in Create root. See [local setup and recovery](services/unified-server/README.md).

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
