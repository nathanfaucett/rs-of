# Ticket 02: Embedded Database Implementation (`embedded-db`)

**Status: ready-for-agent**

**Blocked by:** 01

## What it delivers

A new `embedded-db` crate implementing the `Database` trait from `of-model` using redb. Provides a single-file embedded database at `<data_dir>/unified.redb` with embedded migrations. Used by unified-server and E2E tests.

## Scope

- Create `of/crates/embedded-db/` with `redb` dependency
- Implement `Database` trait:
  - `EmbeddedDatabase` struct wrapping `redb::Database`
  - `txn()` returning redb transaction
  - Owner-specific sub-trait implementations (`IdpDatabase`, `DeviceDatabase`, `StorageDatabase`, `ManagementDatabase`) delegating to redb tables
- Embedded migration runner: call `of_model::replica::up(&db)` on initialization
- Single database file: `<data_dir>/unified.redb`
- No external dependencies beyond redb; `no_std` compatible (with alloc)
- Error handling: convert redb errors to `of_model::error::DatabaseError`

## Acceptance Criteria

- `cargo check -p embedded-db` passes
- `EmbeddedDatabase::open(path)` creates/opens `unified.redb` and runs migration
- All owner database operations work through trait methods
- Unit tests pass using `EmbeddedDatabase` with temp directory
- No `std` dependencies beyond what redb requires
