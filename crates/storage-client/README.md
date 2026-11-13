# Storage client

SQL and KV handles use the current ofdb checked history APIs. Query and transaction
operations remain local. Resource creation, lookup, and selection remain Storage
owner operations; the client does not grant resource access or Installation membership.

- `SqlDatabaseHandle::synchronize` obtains a fresh `storage:sync` token, performs
  the Storage v1 Hello/Attach exchange, and carries SQL v7 owner messages over
  WebSocket. It checks credentials again before each inbound apply. The Storage
  server remains responsible for token introspection and resource authorization.
- `SqlDatabaseHandle::synchronize_with` accepts an already admitted SQL resource
  stream, the current ofdb session configuration and role, a staging directory,
  and an authorization/cancellation check.
- `KvStoreHandle::synchronize_with` accepts an already admitted KV resource stream
  and uses the checked KV v3 exchange. SQL and KV protocols are not interchangeable.
  The Storage WebSocket endpoint currently admits SQL only; this client does not
  invent a KV WebSocket protocol.

For either injected stream, the caller must authenticate the peer, bind the stream
to the handle's resource, and reject revoked authorization or cancellation in the
check callback. A transport identity is not a resource-access grant.

SQL and KV share the Storage database resource catalog. Filesystems use the
filesystem catalog. Token exchange uses the IdP OAuth form endpoint and the exact
Storage resource audience. Both native and browser entry points share this HTTP
implementation; their WebSocket transports remain platform-specific.

Run the focused checks with:

```sh
cargo test -p of-storage-client --all-targets
cargo clippy -p of-storage-client --all-targets -- -D warnings
```
