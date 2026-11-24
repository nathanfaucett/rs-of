# 05: Enforce generated-only UUIDv7 primary keys in the SQL Client

**What to build:** The Automerge-backed SQL engine rejects every caller-supplied single-UUID primary key during local insertion and generates UUIDv7 for omitted or DEFAULT keys. The rule applies equally to SQL text, parameters, typed statements, explicit transactions, multi-row inserts, ON CONFLICT, embedded Clients, and remote ofdb Clients. Replication between updated Databases continues to apply peer-generated UUIDv7 Logical Row identities. Public documentation and fresh-initialization contracts reflect the breaking change without migrations or new protocol versions.

**Blocked by:** 01: Use generated Logical Row identities in Device persistence; 02: Use generated Logical Row identities in IdP persistence; 03: Use generated Logical Row identities in Management persistence; 04: Decouple Storage Resource identities from Logical Row identities

**Status:** ready-for-agent

Work: resolved

- [x] Explicit UUIDv5 and UUIDv7 primary keys are rejected through SQL literals, bound parameters, typed whole-row inserts, and typed column/value inserts.
- [x] Rejection occurs before duplicate checks, Tombstone checks, ON CONFLICT branches, or persistence and uses a clear invalid-query error.
- [x] Omitted and DEFAULT primary keys generate distinct RFC-variant UUIDv7 values that are available through RETURNING and subsequent queries.
- [x] Rejected multi-row inserts leave no partial writes, and explicit NULL retains its existing validation behavior.
- [x] UUID values in ordinary columns, references, predicates, and results remain supported; natural and composite primary keys retain their existing behavior.
- [x] Embedded transactions and remote ofdb execution enforce the same server-side engine contract without changing statement or value wire shapes.
- [x] Fresh updated Replicas synchronize peer-generated UUIDv7 identities through snapshot, state, and native-change paths; legacy UUIDv5 Logical Row data is unsupported.
- [x] Admission distinguishes supplied from generated values before materialization erases their origin, and the existing UUIDv7 generation helper is reused.
- [x] Current documentation removes explicit UUIDv5/v7 primary-key support and states generated-only insertion and fresh-store requirements.
- [x] KV, the independent PostgreSQL backend, and the generic Automerge actor model remain unchanged.
- [x] Focused SQL engine, Automerge, SQL Client, remote, synchronization, and affected service tests pass, followed by required formatting and focused Clippy checks.

## Comments

Implementation integrated. On the integration tip, `cargo test -p ofdb-sql --features sql,redb,validated-history,remote,server --test client --test remote` passed 12 tests with one pre-existing ignore. The engine ticket run passed 107 focused tests with seven ignores before review follow-ups. Review added batch preflight, staged row validation, and `DEFAULT VALUES`. Formatting passed. Broader service, Clippy, feature, and CRAP qualifications are tracked under issue 06.

2026-11-24: The configured SQL Client/remote tests and focused SQL library Clippy passed again. Native and WASM all-feature checks passed. Native `SyncTransport::receive` implementors must now return `Send + 'static` futures that own or share receive state. This is an intentional source-only contract change; wire messages, the protocol version, and persisted data are unchanged, so no migration is needed. The two former KV document failures are fixed; `ofdb-kv-store` passed nine unit tests with its two existing ignores. Formatting and diff checks passed.
