# of — Offline-First Identity, Storage, and Networking

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-MIT)
![Test Status](https://github.com/aicacia/rs-of/actions/workflows/ci.yml/badge.svg)

A monorepo for building offline-first applications with OAuth 2.0 / OpenID Connect identity, local-first databases and filesystems, and peer-to-peer mesh networking.

## Components

| Component             | Directory                                                                        | Description                                                                                                                                              |
| --------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Identity & Access** | `services/idp-service`, `services/management-service`, `services/device-service` | OAuth/OIDC IdP, roles/permissions, device authority                                                                                                      |
| **Storage**           | `services/storage-service`, `services/storage-server`                            | Authenticated database/filesystem APIs, WebSocket transport                                                                                              |
| **Databases**         | [`db/`](db/README.md)                                                            | [`ofdb-sql`](db/README.md#ofdb-sql-relational-sql-engine) (relational, CRDT sync), [`ofdb-kv`](db/README.md#ofdb-kv-key-value-store) (Automerge history) |
| **Filesystem**        | [`fs/`](fs/README.md)                                                            | [`file-system`](fs/README.md) — offline-first filesystem with sync                                                                                       |
| **Networking**        | [`net/`](net/README.md)                                                          | [`iroh-chain`](net/README.md) — mesh networking, pairing, byte streams                                                                                   |
| **TypeScript SDKs**   | [`packages/`](packages/README.md)                                                | WASM wrappers for `@ofsdk/sql`, `@ofsdk/kv`, `@ofsdk/net`, `@ofsdk/file-system`, etc.                                                                    |

## Quick Start

### Local Development Backend

```bash
# Run unified server (all services in one process)
just unified --data-dir .data --port 1355 --origin http://localhost:5173 --no-tls
```

All four owners use one embedded database at `.data/unified.redb`:
`/idp`, `/device`, `/management`, and `/storage`. Each exposes `/health`.

- Local administrator: `admin@local.test` / `password`
- Local sign-in client: `local-login`
- Public PKCE clients: `app` (callback at origin `/callback`) and `password-manager` (port 5174, `/callback`)
- Issuer: backend base + `/idp` (HTTP or HTTPS scheme)
- CORS permits app and password-manager origins

Omit `--no-tls` for HTTPS with self-signed certificate in `.data/tls/`.

### TypeScript SDK

```bash
# Install
pnpm add @ofsdk/sdk

# Or individual packages
pnpm add @ofsdk/sql @ofsdk/kv @ofsdk/net @ofsdk/file-system
```

```typescript
import {
  createSqlClient,
  createKvClient,
  createNetClient,
  createFileSystemClient,
} from "@ofsdk/sdk";

// SQL database
const sql = await createSqlClient({ memory: true });
await sql.execute("CREATE TABLE users (id UUID PRIMARY KEY, name TEXT)");

// KV store
const kv = await createKvClient({ memory: true });
await kv.set("session", { user: "alice", expiry: 3600000 });

// Mesh networking
const net = await createNetClient();
await net.connect(trustedPeer);

// Filesystem
const fs = await createFileSystemClient({ path: "/data" });
await fs.write("/note.txt", new TextEncoder().encode("Hello offline!"));
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      of (this repo)                          │
├──────────────────┬──────────────────┬───────────────────────┤
│  services/       │  db/             │  fs/                   │
│  ├── idp-service │  ├── sql         │  └── file-system      │
│  ├── management  │  ├── kv          │  └── file-system-wasm │
│  ├── device      │  ├── sync        │                        │
│  ├── storage     │  ├── btree*      │  net/                  │
│  └── servers     │  └── wasm        │  └── iroh-chain       │
│                  │                  │  └── iroh-chain-wasm  │
├──────────────────┴──────────────────┴───────────────────────┤
│  packages/ (TypeScript SDKs)                                 │
│  @ofsdk/sql, @ofsdk/kv, @ofsdk/net, @ofsdk/file-system...   │
└─────────────────────────────────────────────────────────────┘
```

### Service Ownership

- **IdP** — Users, clients, tokens, signing keys, OAuth/OIDC
- **Management** — Roles, permissions, assignments
- **Device** — Devices, endpoints, approval/transfer/revocation, resource selection, admin restrictions, sync policy
- **Storage** — Database/filesystem resource APIs, WebSocket transport
- **ofdb** (in `db/`) — SQL/KV persistence, CRDT sync, staging, batching, cleanup
- **offs** (in `fs/`) — Filesystem persistence/sync
- **ofnet** (in `net/`) — Iroh endpoints, mesh/pairing, allowlist, connections/routing

Each Node shares one `iroh-chain` endpoint/router. Endpoint identity and locally trusted membership are persisted and restored by `of`. See [networking composition and interfaces](docs/design.md#networking-composition-and-trust).

## Development

### Prerequisites

- Rust 1.100+ (see `rust-toolchain.toml`)
- Node.js 20+ (for TypeScript packages)
- `wasm-pack` for WASM builds

### Build

```bash
# Rust workspace
just build

# TypeScript packages
just npm-install
just sdk-build

# All tests
just test
```

### Code Quality

```bash
# Format
just fmt-check

# Lint
just clippy

# CRAP (Complexity, Risk, and Priority)
just crap

# Boundary checks
just boundary-check

# Container contract test
python3 scripts/test_container.py
```

## Documentation

- [GLOSSARY.md](GLOSSARY.md) — Domain terminology (Endpoint, Peer, Mesh, Broadcast, Direct Message, Byte Stream, and all ofdb/offs terms)
- [GLOSSARY-MAP.md](GLOSSARY-MAP.md) — Cross-references between contexts
- [docs/design.md](docs/design.md) — System design
- [docs/operations.md](docs/operations.md) — Operational qualification
- [docs/backend-deployments.md](docs/backend-deployments.md) — Deployment boundaries

### Component READMEs

- [db/README.md](db/README.md) — Offline-first databases (`ofdb-sql`, `ofdb-kv`)
- [fs/README.md](fs/README.md) — Offline-first filesystem (`file-system`)
- [net/README.md](net/README.md) — Mesh networking (`iroh-chain`)
- [packages/README.md](packages/README.md) — TypeScript SDK packages

## Deployment

### Unified Server (Recommended)

Single binary, single database file, no external dependencies:

```bash
cargo run -p unified-server -- local --data-dir /var/lib/of --port 443 --origin https://app.example.com
```

### Independent Services

Separate `idp-server`, `device-server`, `storage-server`, `management-server` deployment paths retain their existing configuration. See [configuration](config/README.md) and [operations](docs/operations.md).

Run via portless for local HTTPS:

```bash
just idp
just management
just device
just storage
```

### Docker

```bash
docker build -t of:latest .
docker run -v /var/lib/of:/data -p 443:443 of:latest
```

The Docker image builds from this repo alone — no external repository checkouts required.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
