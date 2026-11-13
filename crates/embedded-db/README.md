# Shared SQL databases

This crate implements the object-safe `of_model::db::Database` interface. It reuses
`idp-db` / `ofdb-sql`; it does not add a second redb schema or transaction protocol.

- `EmbeddedDatabase::open(data_dir).await` creates the directory, opens
  `<data_dir>/unified.redb`, and runs `of_model::replica::up` before returning.
- `MemoryDatabase::new().await` initializes the same schema without a file.
  New instances are isolated. Clones share their instance's data.
- With `network`, `NetworkDatabase::connect(&RemoteDatabaseConfig).await` uses the
  existing ofdb SQL client, then applies the same migration remotely. The server
  must permit schema initialization. This is **not a PostgreSQL adapter**.

All adapters are cloneable and can be stored in `Arc<dyn Database>`. Owner views
return clients for the same database. They are not authorization boundaries.
`txn().await` returns the existing SQL transaction, which can commit or roll back
changes across owner tables. Keep file-backed clones, clients, and transactions
closed before reopening the file with a new database owner.

Initialization is idempotent and transactional. It uses the existing fresh unified
schema, not an upgrade path for old independent owner databases. A failed migration
returns an error, not a usable adapter. This crate does not change setup or wire
owner services into the new catalog; owner-service injection belongs to ticket 03.

## Standard library requirement

This is a native **std-required** runtime crate, including with
`--no-default-features`. redb requires file IO and synchronization. The reused SQL
redb and memory engines also enable `std`; `of-model/replica` requires `std` for SQL
and migrations. Network access requires the existing Tokio runtime and transport.
There is no meaningful alloc-only configuration for these adapters. The separate
`of-model` domain types still build without default features.

The only production dependencies are the existing database and model crates.
redb is used through the SQL engine, not a redundant direct dependency. Network
transport dependencies are optional. `of_model::error::DatabaseError` is the existing
SQL query error under a domain-facing name; its category is retained for SQL,
migration, and redb errors. Directory IO maps to `Storage`; network configuration,
connection timeout, and other connection failures map to `Validation`, `Timeout`,
and `Transport`. Remote SQL errors after connection retain the SQL client's kinds.

The network config retains existing security rules: bare loopback HTTP for local
tests, or HTTPS with a CA certificate and client certificate/key. It retains finite
connection and query deadlines. Live mTLS is not covered by this crate's tests.

## Tests

`cargo test -p embedded-db --all-features --all-targets` uses real redb files,
independent memory databases, and a real loopback ofdb SQL server. It checks all
owner schemas, owner CRUD, cross-owner commit/rollback, reopen persistence, repeated
initialization, failed migrations, invalid files, and rejected network config.
