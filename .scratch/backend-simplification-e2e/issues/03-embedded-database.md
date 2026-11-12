# Ticket 03: Embedded Database Implementation

**Status: ready-for-agent**

**Blocked by:** 02

## What it delivers

An `EmbeddedDatabase` implementation of the `Database` trait using `redb` (single file, no external process). Used by unified-server for zero-config local/E2E runs.

**New crate, no legacy** — clean implementation for unified use case.

## Scope

- Create `of/crates/embedded-db` (or add to `of-db` crate)
- Implement `Database` trait for redb:
  - Single file at `<data_dir>/unified.redb`
  - Schema migrations embedded (run on open)
  - Connection pooling via `redb::Database` (already thread-safe)
  - Transaction mapping to `redb::WriteTransaction`/`ReadTransaction`
- Implement owner-specific tables:
  - IDP: users, clients, sessions, auth codes, keys
  - Device: devices, pairings, credentials
  - Storage: vaults, items, replicas, sync state
  - Management: policies, permissions, audit logs
- Provide `EmbeddedDatabase::open(data_dir)` constructor
- Add `migrate()` method running embedded SQL/redb migrations
- **No feature flag** — always available, unified-server depends on it directly

## Acceptance Criteria

- `EmbeddedDatabase` implements `Database` trait fully
- Opens/creates single `.redb` file
- Runs migrations on first open (idempotent)
- Thread-safe for concurrent owner access
- No external process required
- Integrates with unified-server in next ticket
