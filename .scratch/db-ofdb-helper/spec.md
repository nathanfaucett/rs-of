# Spec: Unified `db` helper crate and `ofdb` naming for the SQL layer

Status: ready-for-agent

## Problem Statement

The layering between the `ofdb` engine, `of-model`, the `db` crate, and `embedded-db` is confusing and looks like duplicated work:

- `embedded-db` looked like a pointless abstraction over `ofdb`, because its job (open engine, run migration, implement the domain database trait) was never stated as one small composition root.
- `of-model` mixed domain structs with the SQL engine: it held the owner-view `Database` trait, the migration runner, and direct `ofdb-sql`/`btree-redb` dependencies.
- The owner-view traits (`IdpDatabase`, `DeviceDatabase`, `StorageDatabase`, `ManagementDatabase`, `Database`) are confusing leftovers from the per-owner-database design. After the schema was unified into one migration file, owner views are logical only; no service code uses them as bounds, only server injection points and tests.
- Per-owner migration folders duplicated the same schema; this was already consolidated into a single init migration applied by every entry point.
- The term `replica` was used for the SQL data layer (modules, runners, features), but in the domain glossary `ofdb` owns SQL persistence and `replica` is an avoid-term. The SQL layer should be called `ofdb`.

## Solution

Make the layers explicit and one-directional:

- `ofdb` (workspace): generic engine only. Unchanged. Owns SQL/KV persistence, sync, protocol.
- `of-model`: pure domain. Structs plus a single migration constant (`ofdb::MIGRATION`, backed by the one init migration file). No engine dependencies, no database trait, no migration runner.
- `db` (crate in the `of` workspace, package renamed to `of-db`, lib name stays `db`): the helper everyone uses. Contains the remote config and transport, the migration runner, the concrete database adapters, and the local open helpers. The separate `embedded-db` crate is deleted; its adapters move here.
- Services: take `Arc<ofdb_sql::Database>` / `Client` directly. No domain database trait exists anymore.
- Servers: build adapters from `db` and inject `Arc<ofdb_sql::Database>` into services.
- Rename the SQL-layer `replica` modules and features to `ofdb` across `of-model`, the services, and `db`. Real replication terminology (`contract::replication`, replica-signer routes) is untouched.

## User Stories

1. As a maintainer, I want one crate that turns a data dir, memory, Postgres URL, or remote endpoint into a ready migrated database, so that I do not copy open-plus-migrate-plus-wrap boilerplate into every server binary.
2. As a maintainer, I want `of-model` to contain only domain structs and the migration SQL, so that it builds `no_std`/wasm and has no engine coupling.
3. As a service author, I want to depend on plain `ofdb` SQL types (`Database`, `Client`) instead of a wrapper trait with owner views, so that the code states exactly which engine it uses.
4. As a server author, I want to inject a concrete `Arc<ofdb_sql::Database>` into my runtime, so that no `dyn` wrapper trait is needed.
5. As a storage server author, I want to get the local `SharedDatabase` from the concrete adapter, so that the file-system runtime keeps working without a trait method.
6. As a reader of the code, I want the SQL layer named `ofdb` everywhere, so that the name matches the glossary and the engine workspace.
7. As a reader of the code, I want replication terminology (`replication`, replica signers) to keep its sync meaning, so that renaming the SQL layer does not blur the two concepts.
8. As a maintainer, I want one migration file applied by every entry point, so that standalone owner databases and the unified host share exactly the same schema.
9. As a test author, I want the schema tests to live with the `db` helper, so that the crate that runs migrations also owns their verification.
10. As a maintainer, I want the package name `of-db` with lib name `db`, so that all existing `db::...` imports keep working after the merge.

## Implementation Decisions

- Crate layout after the change: `ofdb` (engine) -> `of-model` (pure domain) -> `db` helper (lib `db`, package `of-db`) -> services -> server binaries. No cycles; `of-model` never depends on `db`.
- `of-model` keeps: domain structs, error types, and `ofdb::MIGRATION` (the single init migration as an included SQL string). It loses: the `db` module (all owner-view traits and the `database!` macro impls), the replica runner modules, and the engine dependencies behind them. The `replica` feature is renamed `ofdb` and shrinks to what the pure crate needs.
- `db` gains, merged from the deleted `embedded-db` crate: the migration runner `db::ofdb::up(&client)` (single function; the old `up`/`up_device` both applied the same file), the schema validators, the concrete adapters `EmbeddedDatabase`, `MemoryDatabase`, `NetworkDatabase` (Postgres), `RemoteDatabase` (ofdb SQL service), and the local open helpers (`open_private_storage`, `local_database`, `local_client`). Adapters are concrete structs with `client()` and, for file-backed ones, `storage()` accessors; no trait is implemented for them.
- `db` keeps: `remote` config (`RemoteDatabaseConfig`) and `database_transport`, feature-gated as today so light consumers (remote config only, wasm clients) do not pull redb. The adapter/runner code stays behind the native/std features.
- Server runtimes change their injection point from `Option<Arc<dyn of_model::db::Database>>` to `Option<Arc<ofdb_sql::Database>>`. The storage handle is taken from the concrete `db` adapter when present.
- Rename mapping (SQL layer only): `of_model::replica` -> `of_model::ofdb`; `of_model::idp::replica` validators -> `db::ofdb`; `device_service::replica` -> `device_service::ofdb`; `idp_service::replica` -> `idp_service::ofdb`; `management_service::replica` -> `management_service::ofdb`; feature `replica` -> `ofdb` on `of-model`, `db`, `device-service`, `idp-service`, `management-service`, and in workspace/app manifests. Runner call sites move from `of_model::replica::{up, up_device}` to `db::ofdb::up`.
- Untouched: `of_model::contract::replication`, replica-signer HTTP routes, ofdb sync internals, the ofdb engine itself, and the already-landed single init migration file.
- Schema: already unified. One init migration creates all owner tables; every entry point (unified host, standalone idp, standalone device) applies it. Standalone owner database files intentionally contain the full schema; owner separation is logical (which tables a service writes), not missing tables.
- Open naming point: module name `ofdb` sits next to the `ofdb_sql` crate import in some files. Accepted per glossary vocabulary; alternative `sql` was considered and rejected in favor of `ofdb`.

## Testing Decisions

- Test at the highest existing seams; add no new seams:
  - `db` crate integration tests (moved from `embedded-db` and `of-model`): open/migrate/persist/reopen for file and memory adapters, all owner tables reachable through the single shared client, cross-owner transaction commit and rollback, repeated initialization idempotency, failed migration returns error, unsafe existing files rejected, remote loopback server test, Postgres test ignored unless `OF_TEST_POSTGRES_URL` is set.
  - Server constructor tests (existing `local.rs` tests in each `*-server`): injected shared database and owner-file fallback both build routers without setup.
- Tests assert external behavior only: tables exist, reads/writes round-trip, transactions commit or roll back, files persist across reopen. No assertions on internal trait structure (the trait is gone).
- Prior art: the existing `embedded-db` test suite and the `unified_schema` test in `of-model`; both move to `db` with the rename.
- Final validation once the `ofdb` working tree compiles again: feature-powerset tests for `of-model` and `of-db`, then workspace tests including the server crates. Until then, `of-model` must compile with no default features.

## Out of Scope

- Renaming or refactoring real replication/sync code (`contract::replication`, replica signers, ofdb sync).
- Any change to the `ofdb` engine workspace itself, including its in-flight refactor.
- Reintroducing owner-view traits or per-owner database files/schemas.
- Authorization semantics: owner views were never authorization boundaries; nothing about access control changes.
- Changing the wire protocol, the remote config format, or Postgres behavior.

## Further Notes

- The single-migration consolidation is already landed: one init migration file, applied by all entry points, with external `include_str!` consumers updated.
- The `ofdb` workspace working tree currently does not compile (`ofdb-sql-schema`, `ofdb-btree-redb` unresolved imports from a concurrent refactor). This blocks full workspace test runs; it is pre-existing and not caused by this work. Track it separately.
- Expected blast radius of the rename: ~380 `replica` occurrences across ~94 files in `of/`, of which the replication/sync subset (~27 lines) must be excluded.
