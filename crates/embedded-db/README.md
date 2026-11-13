# Shared SQL databases

This crate implements the object-safe `of_model::db::Database` interface. It reuses
`idp-db` / `ofdb-sql`; it does not add a second redb schema or transaction protocol.

- `EmbeddedDatabase::open(data_dir).await` creates the directory, opens
  `<data_dir>/unified.redb`, and runs `of_model::replica::up` before returning.
- `MemoryDatabase::new().await` initializes the same schema without a file.
  New instances are isolated. Clones share their instance's data.
- With `postgres`, `NetworkDatabase::connect(connection_string).await` connects
  to PostgreSQL, then applies the same owner migration. The database role must
  permit schema initialization. Use a Tokio runtime with IO enabled.
- With `network`, `RemoteDatabase::connect(&RemoteDatabaseConfig).await` uses the
  independent ofdb SQL service. Its protocol and security rules are unchanged.

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

## PostgreSQL

```rust,ignore
let database = embedded_db::NetworkDatabase::connect(
    "host=/run/postgresql dbname=of user=of sslmode=disable connect_timeout=5",
).await?;
let shared: &dyn of_model::db::Database = &database;
let transaction = shared.txn().await?;
transaction.rollback().await?;
```

The adapter uses `db::Client` and its PostgreSQL backend, not the ofdb network
protocol. SQL is parsed into the existing query representation, then compiled to
quoted PostgreSQL identifiers and bound values. UUID, boolean, integer, float,
text, binary, JSON, and null values retain their shared query types. Unsupported
query operations return `Unsupported`; this is not an arbitrary PostgreSQL SQL
console.

Each transaction owns a connection at serializable isolation. SQL or translation
failure prevents commit. Dropping a transaction closes its connection and rolls
back its writes. Callers must retry serialization failures at their operation
boundary. PostgreSQL has no retained CRDT conflicts; conflict inspection validates
the table and key, then returns no conflicts. Native read limits and CRDT conflict
resolution are unsupported. `local_storage()` returns `None`.

The current driver uses no TLS. It rejects the default `sslmode=prefer` and
requires explicit `sslmode=disable`. Use a private Unix socket or a trusted TLS
tunnel; do not send credentials over an untrusted plaintext network. Set
`connect_timeout` in the connection string and use `Client::with_deadline` when
query deadlines are required. PostgreSQL credentials must come from deployment
secrets, not source control.

The ofdb remote config retains existing security rules: bare loopback HTTP for local
tests, or HTTPS with a CA certificate and client certificate/key. It retains finite
connection and query deadlines. Live mTLS is not covered by this crate's tests.

## Tests

`cargo test -p embedded-db --all-features --all-targets` uses real redb files,
independent memory databases, and a real loopback ofdb SQL server. It checks all
owner schemas, owner CRUD, cross-owner commit/rollback, reopen persistence, repeated
initialization, failed migrations, invalid files, and rejected network config.

The live PostgreSQL test is ignored by default. Run it against a disposable
PostgreSQL database (it creates the shared schema):

```sh
OF_TEST_POSTGRES_URL='host=127.0.0.1 port=5432 user=of dbname=of_test sslmode=disable connect_timeout=5' \
  cargo test -p embedded-db --features postgres --test postgres -- --include-ignored
```

It checks all four owner views, cross-owner commit and rollback, repeated
migration, and persistence after reconnect. The `ofdb-sql` PostgreSQL integration
test also checks typed values, failed transactions, and rollback on drop.
