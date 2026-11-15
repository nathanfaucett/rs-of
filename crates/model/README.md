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

`error::DatabaseError` (with `replica`) re-exports the existing SQL `QueryError`.
`embedded-db` provides initialized Embedded, Memory, and optional Network adapters
for this interface. Its runtime requires `std`; domain-only builds do not.
See [the adapter README](../embedded-db/README.md).

There is a single migration: `migrations/0001_init.up.sql` holds the full schema (identity, Device, Management sync, and Storage catalog tables). `replica::up`, `idp::replica::up`, and `replica::up_device` all apply it in one SQL transaction; owner separation is logical (the `Database` owner views and per-owner validation), not separate table sets. The integration test uses a real redb file, repeats initialization, commits and rolls back transactions, then checks retained data after reopen.

`idp::replica::up` additionally rejects legacy mixed credential history, and Device schema validation stays in `device-service`. Independent Storage keeps its existing file-backed catalogs; the SQL catalog is not a replacement for that runtime yet.

## Runtime boundary

The removed `api` crate mixed config with server runtime helpers. Config is now here. Serving and OpenAPI security helpers are in `server-common`. TLS, setup files, admission, and setup contracts remain in the unchanged `setup` crate. Making this crate depend on setup would create a cycle through setup's generated HTTP clients.

The OpenAPI generator publishes Rust wire models into `http` and writes named re-exports into each client. Its rollback covers both destinations. Generated model code is converted to alloc imports before publication. Full OpenAPI regeneration was not run for this change; the extraction test and cold-cache rollback tests passed.
