**Status:** resolved

- [x] `db` exposes all four adapters plus the local helpers under their new names
- [x] Each adapter opens its engine, runs `db::ofdb::up`, and returns errors instead of a half-initialized adapter
- [x] All four server runtimes accept `Arc<ofdb_sql::Database>` injection and build with or without an injected database
- [x] Storage runtime still receives the local `SharedDatabase` via the concrete adapter's `storage()`
- [x] No `dyn` domain-database trait exists anywhere in the workspace

## Answer

Merged adapters from deleted `embedded-db` into `db` crate:

- Created `db::ofdb` with migration runner `up(&Client)` and validators (`validate_credential_history`, `validate_identity`, `SecurityTable`, `SecurityRow`, `ResolutionAudit`)
- Created `db::local` with `open_private_storage`, `local_client`, `local_database` (accepts optional injected `Arc<SqlDatabase>` for shared DB case)
- Created `db::adapters` with concrete `EmbeddedDatabase`, `MemoryDatabase`, `NetworkDatabase` (Postgres), `RemoteDatabase` — all with `client()` and file-backed with `storage()`
- Updated all four server runtimes + unified-server: injection type changed from `Option<Arc<dyn of_model::db::Database>>` to `Option<Arc<ofdb_sql::Database>>`; use `db::local_database(data_dir, injected)` and `db::local_client(data_dir, injected)`
- Server Cargo.toml: depend on `db` (idp-db) with `native` feature; removed `embedded-db` dependency
- `unified-server` uses `db::adapters::EmbeddedDatabase` and passes shared `SqlDatabase` client to sub-servers

Workspace lib check passes.
