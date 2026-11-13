# Ticket 02: Embedded Database Implementation (`embedded-db`)

**Status: claimed**

**Branch:** `backend-simplification-embedded`
**Base:** `backend-simplification` (`fe234418`)

## Claim

Implement the existing object-safe SQL database boundary over redb, memory, and the existing network client. No setup changes. Test at the Database trait boundary selected by the spec: persistence, shared owner schema, transactions, memory isolation, and real network access.

Complexity: low adapter complexity; medium migration and transaction risk. Priority: blocks ticket 03.

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

## Answer — 2026-11-12

Status remains **claimed**, not resolved. The functional database checks pass, but the original alloc-only requirement and ticket 01 dependency are not satisfied.

### Delivered

- Added workspace crate `crates/embedded-db` using the existing SQL layer over redb. No redundant direct redb dependency or new key/value schema.
- `EmbeddedDatabase::open(data_dir).await` creates the directory, opens only `unified.redb`, and awaits `of_model::replica::up` before returning. Reopen repeats idempotent transactional initialization.
- `MemoryDatabase::new().await` initializes the same schema. New instances are isolated; clones share data.
- Optional `network` feature: `NetworkDatabase::connect(&RemoteDatabaseConfig).await` reuses the existing SQL client, security validation, mTLS configuration, and deadlines. It initializes the remote unified schema. No new transport or PostgreSQL driver.
- All three implement the ticket 01 object-safe Database and owner-view traits. Transactions reuse `db::Transaction`; owner clients share one database and retain existing error categories.
- Added `of_model::error::DatabaseError` as a re-export of the existing `QueryError`, gated by `replica`. Directory errors map to Storage; connection validation/timeout/transport failures map to the corresponding query categories.
- Added five local integration tests and two network integration tests. Tests cover one real file, directory creation, repeated migrations, all owner tables, IdP/Device/Storage/Management CRUD, cross-owner commit and rollback, all-owner data after reopen, memory isolation and clones, failed migration rollback, invalid-file preservation, real network commit/rollback/reconnect, and rejected unsafe network config.
- Documented the runtime boundary in `crates/embedded-db/README.md` and the model README. Added a separate ticket 01 assessment of domain contract duplication versus runtime-only setup types. No setup content or unrelated model types changed.

### TDD evidence

Each new adapter started with its public integration test: EmbeddedDatabase, then MemoryDatabase, then NetworkDatabase. Each failed with the missing public adapter import before implementation, then passed. Additional regression tests exercise the existing SQL and error behavior through the selected public database boundary; they did not require changes to the reused SQL engine.

### Passed validation

Commands ran in `.scratch/worktrees/embedded`, using `--target-dir ../model/target` to reuse build artifacts. Existing sibling links resolved `../ofdb`, `../ofnet`, and `../offs` dependencies.

- `cargo check -p embedded-db`.
- `cargo check -p embedded-db --all-features`.
- `cargo test -p embedded-db --all-features --all-targets`: 7 integration tests passed.
- `cargo hack test -p embedded-db --feature-powerset --all-targets`: all 4 feature runs passed, including the network tests in network-enabled runs.
- `cargo clippy -p embedded-db --all-features --all-targets --no-deps -- -D warnings`.
- `cargo check -p of-model --no-default-features`: passed, with existing unused-import warnings.
- `cargo test -p of-model --features replica --test unified_schema`: 3 tests passed. The first combined validation command timed out after 200 seconds as this test binary started; a separate rerun passed.
- `cargo fmt -p embedded-db -p of-model -- --check` and `git diff --check`.
- Setup diff is empty.

### Exact gaps and failed broad validation

- The runtime is **std-required**, even with no default features. redb, the reused SQL redb/memory engines, and model SQL migrations need std; optional networking also uses Tokio. This meets the requested practical SQL reuse but not the ticket's literal alloc-only scope or restriction of std use to redb alone. Domain-only `of-model` remains alloc-compatible. No new production dependencies beyond existing model/database crates were added.
- NetworkDatabase uses the existing ofdb SQL network service, **not PostgreSQL**. The wider spec's PostgreSQL implementation is not delivered. Live mTLS, timeout fault injection, and independent deployment runtimes were not tested; the existing network config and deadline implementation were reused.
- SQL owner views and the unified catalog work, but owner-service injection and replacing independent Storage's existing runtime catalogs belong to ticket 03. These tests do not claim owner HTTP/runtime qualification.
- Ticket 01 remains claimed: there are real duplicate domain/wire contracts as well as unrelated runtime setup types. See its appended assessment. No domain reshuffle was needed for ticket 02.
- Required `cargo hack test --feature-powerset --workspace --all-targets` failed on of-model run 3/202: `crates/of-model/src/idp/replica.rs:765` implements missing `ofdb_sql::SyncTransport` in retained replica tests. The alloc-only run passed 17 unit tests and 3 contract tests before this failure. This is the retained sync API issue already recorded in ticket 01, not an adapter failure.
- `cargo check --workspace` failed in `offs/crates/file-system/src/sync.rs:15`: `KvSnapshot` lacks serde Serialize/Deserialize (4 errors). This is the offs/ofdb issue recorded in ticket 01.
- Required `just crap` timed out after 60 seconds during coverage compilation, after SDK builds completed. No CRAP score or coverage pass was obtained.

The feature branch is committed and integrated into `backend-simplification` before handoff. No ticket is marked resolved on partial acceptance.
