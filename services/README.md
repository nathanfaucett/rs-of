# services/ — Rust Service Crates

Application services and servers for the `of` ecosystem.

## Services

| Crate | Purpose |
|-------|---------|
| `idp-service` | OAuth 2.0 / OpenID Connect identity provider — users, clients, tokens, signing keys |
| `management-service` | Roles, permissions, assignments |
| `device-service` | Device authority — devices, endpoints, approval/transfer/revocation, resource selection, sync policy |
| `storage-service` | Authenticated database/filesystem resource APIs |
| `storage-server` | WebSocket transport for storage API |
| `idp-server` | OIDC/OAuth 2.x HTTP server |
| `management-server` | Management API HTTP server |
| `device-server` | Device API HTTP server |
| `unified-server` | All services in one binary (local development) |
| `model` | Shared domain models and contracts (`of-model`) |
| `server-common` | Shared server utilities (HTTPS, OpenAPI, setup) |
| `key` | Key management, keyring integration |
| `setup` | Independent node setup, HTTPS, CA |
| `cli` | Shared CLI infrastructure |
| `tauri-plugin-fetch-api` | Tauri plugin for fetch API |
| `*-client` | Generated HTTP clients for each service |

## Feature Flags

Most services support:

- `std` — Standard library (default)
- `ofdb` — Enable database persistence (`ofdb-sql`, `ofdb-kv`)
- `utoipa` — OpenAPI documentation
- `independent` — Standalone deployment (no unified server)

## Testing

```bash
# All service tests
cargo test --workspace -p idp-service -p management-service -p device-service -p storage-service

# With all feature combinations
cargo hack test --feature-powerset -p idp-service --all-targets
```

## Architecture

```
services/
├── idp-service         → IdP owner (identity, OAuth/OIDC)
├── management-service  → Management owner (roles, permissions)
├── device-service      → Device owner (devices, endpoints, sync policy)
├── storage-service     → Storage owner (databases, filesystems)
├── *-server            → HTTP servers for each owner
├── unified-server      → All owners in one process
├── model               → Shared contracts (of-model)
├── server-common       → Shared HTTPS, OpenAPI, setup
└── *-client            → Generated clients
```

Each service owns its database schema and sync protocol. Services communicate via generated HTTP clients (`of-idp-client`, `of-management-client`, etc.) and shared `of-model` contracts.

## Related

- [db/README.md](../db/README.md) — Database crates used by services
- [fs/README.md](../fs/README.md) — Filesystem crate used by storage-service
- [net/README.md](../net/README.md) — Networking crate used by device-service
- [packages/README.md](../packages/README.md) — TypeScript SDKs generated from services
