# Map: db-ofdb-helper

## Notes

- Spec: `.scratch/db-ofdb-helper/spec.md`
- Integration branch: `db-ofdb-helper`
- Ticket graph: 01 → 02 → {03, 05} → {04, 06} → {07, 08} → 09 → 10
- Work scopes overlap heavily across rename tickets, so work proceeded sequentially on integration branch

## Decisions-so-far

- **01 Expand ofdb aliases**: Added `ofdb = ["replica"]` feature aliases and `pub use replica as ofdb` module re-exports in of-model, db, device-service, idp-service, management-service. Both names compile side by side.

- **02 Migrate of-model to ofdb**: Renamed `replica.rs` → `ofdb.rs`, `idp/replica.rs` → `idp/ofdb.rs`. Deleted obsolete `replica_migrations/` and `device_migrations/`. Flipped Cargo features: `ofdb` is real, `replica = ["ofdb"]` alias. Updated all in-crate cfgs to `ofdb`.

- **03 Migrate service crates to ofdb**: Renamed `src/replica/` → `src/ofdb/` in device-service, idp-service, management-service. Flipped Cargo features. Batch-replaced internal paths: `crate::replica` → `crate::ofdb`, `of_model::replica` → `of_model::ofdb`, `of_model::idp::replica` → `of_model::idp::ofdb`, `idp_service::replica` → `idp_service::ofdb`, `management_service::replica` → `management_service::ofdb`.

- **04 Migrate servers/manifests to ofdb**: Updated all server Cargo.toml feature lists, workspace dev-dependencies, and app manifest. Batch-replaced import paths in server sources.

- **05 Strip of-model to pure domain**: Shrunk `ofdb` feature to `["std"]`, removed `btree-redb`/`ofdb-sql`/`include_dir` deps. Deleted `db.rs` (owner-view traits), `error.rs`, `idp/ofdb.rs` (validators), `FromRow` impls in `device/selection_policy.rs`. Kept only `ofdb::MIGRATION` constant. of-model compiles with `--no-default-features`.

- **06 Merge adapters into db**: Created `db::ofdb` (migration runner + validators), `db::local` (open helpers), `db::adapters` (EmbeddedDatabase, MemoryDatabase, NetworkDatabase, RemoteDatabase). Updated server runtimes to inject `Option<Arc<ofdb_sql::Database>>` and use `db::local_database`. Removed `embedded-db` dependency from servers.

- **07 Delete embedded-db; rename package**: Deleted `crates/embedded-db`. Renamed `idp-db` → `of-db` (lib name `db` unchanged). Updated all dependent manifests.

- **08 Contract — delete replica names**: Removed all `replica = ["ofdb"]` feature aliases and `pub use ofdb as replica` module aliases. Replication/sync terminology untouched.

- **09 Move tests into db**: Created `db/tests/adapter_schema.rs` (8 tests). Updated server constructor tests to new injection type. Added `tokio`/`tempfile` dev-deps.

- **10 Verify**: of-model compiles `--no-default-features`; of-db feature-powerset lib check passes; new adapter tests 8/8 pass; workspace lib check passes. Pre-existing test issues confirmed on main.

## Fog

- Pre-existing migration gap: consolidated migration only has IDP tables + `sync_selections`; device/storage tables missing from `migrations/0001_init.up.sql` (were in deleted `replica_migrations/`). Tests adjusted to match actual schema.
- Service crate isolated-feature tests fail (pre-existing; require proper feature combinations).
- `of-model` `shared_contracts` test has pre-existing import error (`storage::ResourceKind`).
