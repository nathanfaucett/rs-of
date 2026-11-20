# Storage client

Current Rust client for Storage resource APIs and local SQL/KV handles. Resource catalogs and access belong to Storage. Selection is desired Device replication state, not a Resource-access grant.

## Current behavior

- SQL and KV queries and transactions operate on local ofdb handles.
- `SqlDatabaseHandle::synchronize` uses the configured token provider and native or browser WebSocket transport. It performs the Storage v1 Hello/Attach exchange and carries SQL v7 messages through the checked SQL history API.
- `SqlDatabaseHandle::synchronize_with` accepts an authenticated SQL stream, session configuration, role, staging directory, and authorization/cancellation check.
- `KvStoreHandle::synchronize_with` accepts an authenticated KV stream and uses the checked KV v3 exchange. The Storage WebSocket endpoint accepts SQL only; it is not a KV protocol.

Callers must authenticate injected streams, bind them to the correct Resource, and reject revoked authorization or cancellation in the check callback. Transport identity is not Resource authority. Use persistent store/replica identities and a real schema fingerprint; an attached session is not proof of correct identity, completed synchronization, or backup.

The target public ofdb boundary is only `ofdb-sql` and `ofdb-kv`: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Current client dependencies are not fully migrated to that boundary.

## Focused checks

```sh
cargo test -p of-storage-client --all-targets
cargo clippy -p of-storage-client --all-targets -- -D warnings
```
