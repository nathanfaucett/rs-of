# Services

Rust domain services, HTTP servers, composition hosts, clients, and support crates.

See the [vision and milestones](../docs/goal.md) and [target architecture](../docs/architecture.md). Current implementation has not completed that target.

## Target ownership

| Context | Responsibility |
| --- | --- |
| IdP | Users, credentials, Applications, OAuth Clients, grants, and Node Signer trust |
| Management | Roles, permissions, and assignments; authorization of administration |
| Device | Device records, endpoint identity, approval/revocation, and Device configuration |
| Storage | Resource catalogs, `(User subject, Application)` namespaces, and Resource access |

Management does not own Device records. A Device is an Installation-level approved endpoint. With permission, it may hold replicas for multiple Users and Applications. Selection maps a Resource to a Device as desired replication state; it does not change Resource ownership or replace access checks. Device revocation neither transfers Resource ownership nor remotely erases copies.

Current contracts and code still contain the legacy single-User Device owner/transfer model. That model is target for removal. Its presence does not qualify shared Installation Devices or completed migration.

## Main workspace directories

| Group | Crates |
| --- | --- |
| Domain services | `idp-service`, `management-service`, `device-service`, `storage-service` |
| HTTP and transport servers | `idp-server`, `management-server`, `device-server`, `storage-server` |
| Composition | `unified-server` |
| Shared contracts and support | `model`, `server-common`, `setup`, `key`, `cli`, `db` |
| Rust clients | `idp-client`, `management-client`, `device-client`, `storage-api-client`, `storage-client` |
| Desktop integration | `tauri-plugin-fetch-api` |

OpenAPI files under `openapi/` are the HTTP contract source. Keep route and schema details there rather than duplicating them in service documentation.

## Database and deployment boundaries

The target public ofdb boundary is only `ofdb-sql` and `ofdb-kv`: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Services must not import internal engine or redb crates. The current dependency graph is not fully migrated.

One server process does not require one physical database file or one shared database host.

## Focused checks

```sh
cargo test -p idp-service -p management-service -p device-service -p storage-service
cargo hack test --feature-powerset -p idp-service --all-targets
```

See [CONTRIBUTING.md](../CONTRIBUTING.md) for workspace validation.
