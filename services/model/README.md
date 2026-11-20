# Shared models

`of-model` contains shared domain types and wire contracts. It has no default features and uses `core` and `alloc` where enabled dependencies permit.

## Current modules

| Module | Contents |
| --- | --- |
| `config` | `Environment` and `ServerConfig` |
| `contract` | Common token, health, authorization, and replication payloads |
| `device` | Device, enrollment, pairing, approval, restriction, and selection types |
| `idp` | Identity entities and OAuth/OIDC contracts |
| `management` | Management roles, permissions, assignments, and synchronization payloads |
| `storage` | Resource identities, namespaces, catalogs, sessions, and synchronization payloads |
| `server` | HTTP contracts shared by router handlers |
| `http` | Generated Rust wire models and re-exports used by generated clients |
| `setup`, `node_join`, `oauth_user` | Setup, Join, and User payload contracts |
| `sync_selection` | Selection repository and service traits |
| `ofdb` | The shared SQL migration text exported as `MIGRATION` |

The crate also contains `id`, error, JSON, and parsing helpers. With the `ofdb` feature, `idp::replica` applies the current IdP and Management schema through `ofdb_sql::Client` transactions.

## Features

- `std`: standard-library support.
- `ofdb`: SQL migration helpers through `ofdb-sql`; enables `std`.
- `utoipa`: OpenAPI schema support; enables `std`.
- `axum`: Axum identity error responses.
- `wasm`: Wasm serialization bindings.

OpenAPI files under `openapi/` are the HTTP contract source. Generated `http` modules are consumers of that contract, not a second hand-written API specification.

The target ofdb boundary is `ofdb-sql`/`ofdb-kv`: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Services must not consume internal engine or redb crates. Migration to that boundary is incomplete.
