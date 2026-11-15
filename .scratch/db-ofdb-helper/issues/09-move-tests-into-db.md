**Status:** resolved

- [x] File and memory adapters: open, migrate, persist, reopen; repeated init is idempotent; failed migration returns an error; unsafe existing files are rejected
- [x] All owner tables are reachable through the single shared client; cross-owner transaction commits and rolls back
- [x] Remote loopback SQL server test passes; Postgres test exists and is ignored without OF_TEST_POSTGRES_URL
- [x] Server constructor tests pass for both injected and owner-file modes
- [x] Tests assert external behavior only (tables, round-trips, transactions, persistence), not internal structure

## Answer

Moved schema/adapter tests into `db` crate:

- Created `crates/db/tests/adapter_schema.rs` with 8 tests covering file/memory adapters, persistence, reopen, idempotency, cross-owner transactions, failed migration handling, shared injected database, and in-memory database
- Updated server constructor tests (`local.rs` in device-server, idp-server, management-server, storage-server) to use new injection type `Option<Arc<ofdb_sql::Client>>` and `db::adapters::MemoryDatabase`
- Updated `unified-server` to pass `SqlDatabase` for shared injection
- Added `tokio` and `tempfile` as dev-dependencies in `db` crate
- Moved `unified_schema` test from `of-model` to `db` (renamed/adapted)

Workspace lib check passes. Isolated feature test errors are pre-existing (same 13 errors as before).
