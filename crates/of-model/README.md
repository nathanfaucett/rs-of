# Shared models

`of-model` has no default features. Domain types and HTTP payloads use `core` and `alloc`.

## Modules

- `contract`: common token, health, authorization, and replication payloads.
- `idp::{model, contract}`: identity entities and OAuth/OIDC payloads.
- `device`: Device, enrollment, pairing, and selection records.
- `storage`: storage protocols, namespaces, resource identities, and database records.
- `management` and `sync_selection`: management sync payloads and repository contracts.
- `server`: owner HTTP contracts used by router handlers and OpenAPI schemas.
- `http`: generated Rust wire payloads. Rust clients re-export these definitions rather than defining their own models. Wire payloads retain their generated constructors and nullable-field representation.
- `config`: Environment and ServerConfig. Reading `PORT` requires `std`; without `std`, the default port is 3000.

## Features

- `std`: standard-library support.
- `utoipa`: OpenAPI schemas; enables `std`.
- `axum`: identity error responses for Axum.
- `wasm`: existing Wasm bindings.
- `replica`: SQL access and migrations; enables `std`.

## Database access

`db::Database` is object-safe. It returns the existing SQL `Client` and `Transaction`, not a second transaction protocol. Implementations cover the existing database owner (redb or memory) and a query `Client` (embedded or remote). Owner sub-traits expose query clients; they do not grant authorization or expose database hosting controls. Owner services retain their existing permission checks.

`replica::up` applies the fresh unified schema in one SQL transaction. It includes identity, Device, Management sync, and Storage catalog tables. The integration test uses a real redb file, repeats initialization, commits and rolls back transactions, then checks retained data after reopen.

Independent identity initialization remains at `idp::replica::up`. Independent Device initialization uses `replica::up_device` and retains Device schema validation in `device-service`. Neither initializes the other owner's tables. Independent Storage keeps its existing file-backed catalogs; the new unified SQL catalog is not a replacement for that runtime yet.

## Runtime boundary

The removed `api` crate mixed config with server runtime helpers. Config is now here. Serving and OpenAPI security helpers are in `server-common`. TLS, setup files, admission, and setup contracts remain in the unchanged `setup` crate. Making this crate depend on setup would create a cycle through setup's generated HTTP clients.

The OpenAPI generator publishes Rust wire models into `http` and writes named re-exports into each client. Its rollback covers both destinations. Generated model code is converted to alloc imports before publication. Full OpenAPI regeneration was not run for this change; the extraction test and cold-cache rollback tests passed.
