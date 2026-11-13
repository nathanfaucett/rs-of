# Ticket 01: Extract Single Model Crate (`of-model`)

**Status: claimed**

**Owner:** backend-simplification-model (`.scratch/worktrees/model`)

**Blocked by:** (none)

## What it delivers

A single `of-model` crate containing all shared domain types, OpenAPI contracts, config shapes, error types, and the `Database` trait. All server crates and clients depend only on this crate for shared types. Eliminates duplicate definitions across `api`, `idp-model`, `storage-model`, `management-model`.

**Breaking changes expected** — rename types, move modules, change signatures. No compat shims.

## Scope

- Create new `of/crates/of-model` (no_std + serde features)
- Move from `api`: contract definitions, `LocalTls`, `ServerConfig`, setup file utilities
- Move from `idp-model`: all identity/domain types, OAuth2/OIDC contracts, replication types
- Move from `storage-model`: all storage/domain types, sync protocols
- Move from `management-model`: management types, permission contracts
- Define `Database` trait in `of-model/src/db.rs` with owner-specific sub-traits
- Write unified migration `replica_migrations/0001_init.up.sql` concatenating all existing migrations in dependency order
- Update all dependent crates (`idp-server`, `device-server`, `management-server`, `storage-server`, `unified-server`, clients) to import from `of-model`
- **Delete** duplicate type definitions from source crates
- **Delete** `idp-model`, `storage-model`, `management-model` crates if empty after move
- **Delete** `api` crate (contracts moved to `of-model`)

## Acceptance Criteria

- `cargo check --workspace` passes
- No type defined in more than one crate
- All OpenAPI contracts compile from single source (`of-model`)
- `of-model` builds with `no_std` (alloc only)
- Clients (`idp-client`, `management-client`, `storage-client`) use `of-model` types
- Old model crates removed or empty
- `of_model::replica::up()` runs migration against empty redb without error

## Implementation — 2026-11-12

Status remains **claimed**. Do not resolve this ticket on the checks below alone.

### Risk assessment

High complexity and risk: shared type identity, feature flags, generated clients, owner migrations, and setup dependencies all change together. High priority: other backend-simplification tickets need this crate. Preserve owner runtime behavior and setup content. No CRAP score was obtained; the required command timed out.

### Delivered

- Added `crates/of-model`, with no default features. Shared identity, storage, Device, selection, Management sync, config, and error types use alloc. SQL and migration IO require `replica`, which enables `std`.
- Removed `idp-model`, `storage-model`, and `model`. There was no `management-model` crate to remove. Updated Rust dependencies, imports, and SQL fixture paths across crates, apps, examples, and tests. No dependency aliases retain removed crate names.
- Moved generated Rust wire types from all five client crates into `of_model::http`. Clients now re-export named shared definitions. Moved owner router payloads into `of_model::server`; retained wire serialization and validation attributes.
- Deleted `api`. Config moved into the model. Serving and OpenAPI runtime helpers moved into `server-common`; unused API middleware was deleted. TLS and setup utilities remain in the unchanged setup crate.
- Added the object-safe `of_model::db::Database` trait and owner views. Existing SQL Database and Client implement it. Transactions are real `db::Transaction` values, not a new transaction protocol. Embedded, memory, and remote query access reuse ofdb.
- Added transactional unified initialization: identity, Device, Management sync, and fresh Storage catalog tables. Kept independent identity and Device migrations separate. Independent Storage still uses its file-backed catalogs; the new unified catalog has no runtime integration in this ticket.
- Updated the OpenAPI generator and Cargo template to publish shared Rust models and client re-exports together. Rollback tests include the shared model destination.

### Passed checks

- `cargo check -p of-model --no-default-features`.
- `cargo test -p of-model --no-default-features --lib --test shared_contracts`: 17 unit tests and 3 contract tests.
- `cargo test -p of-model --features replica,utoipa,axum --test shared_contracts --test unified_schema`: 4 contract/schema tests and 3 database tests. The tests check real-redb initialization, repeat initialization, all owner tables, commit, rollback, data after reopen, memory query-client access through `dyn Database`, and independent owner schema separation.
- `cargo check -p of-model --all-features -p server-common -p of-idp-client -p of-device-client -p of-management-client -p of-storage-api-client`.
- `cargo check -p idp-service -p management-service --features replica`.
- `cargo check -p device-service --no-default-features`.
- `cargo clippy -p of-model --all-features --lib`: completed with warnings, not warning-free.
- `cargo fmt -p of-model -p server-common -- --check`.
- `python3 scripts/check_server_dependencies.py`.
- `node --test scripts/shared-rust-models.test.mjs`: 1 test.
- `node --test --test-name-pattern='cold SDK cache' scripts/generate-openapi-client.test.mjs`: 2 tests.
- `git diff --check`; no setup content changes.

### Unresolved acceptance and limits

- `cargo check --workspace` did not finish within 200 seconds. It is not a verified pass.
- Device replication compilation has 62 errors in existing async transport, bootstrap, and candidate-view code. Reproduced the same 62 errors on untouched base commit `97677e16b` in a detached baseline worktree.
- `of-storage-client` fails on missing `storage_sync_client` and removed `ofdb_sync::{PushResult, SyncRole}` imports. Reproduced the same 14 errors on the base commit. Its shared model source was moved, but the full client does not compile.
- The retained identity replication unit tests use removed ofdb sync APIs. The baseline `cargo test -p idp-model --features replica,utoipa,axum` also fails. The alloc-only model tests and new SQL integration tests pass; the full replica unit suite does not.
- `idp-server` already has an extra closing delimiter in `router/routes/administration.rs`; checked against the base source. Rustfmt cannot format that file. This ticket preserves that unrelated code.
- Standalone `storage-service` checking also reaches existing offs/ofdb `KvSnapshot` serde errors. Full server and independent runtime behavior is not verified.
- `LocalTls`, setup file utilities, setup contracts, and setup-bound independent join contracts remain outside `of-model`. Moving them would require changing setup content or breaking its dependency cycle through generated clients. Thus the strict all-types/all-OpenAPI single-source acceptance is not satisfied.
- `cargo hack test --feature-powerset --workspace --all-targets` timed out after 200 seconds. `just crap` timed out after 60 seconds during compilation. Neither is a pass.
- Full OpenAPI regeneration, live independent runtimes, PostgreSQL support, and owner-service injection through the new Database trait were not validated or implemented here. Existing services still use their existing query clients; runtime integration belongs to later tickets.

The detached baseline worktree was used only to reproduce existing failures. The implementation worktree and branch remain `.scratch/worktrees/model` and `backend-simplification-model`.

## Ticket 02 single-source assessment — 2026-11-12

Ticket 02 does not resolve ticket 01 or change setup. A public struct/enum name scan across `crates/`, followed by source inspection, separates these cases:

- **Actual contract duplication:** `idp-service/src/oauth2/service.rs` still defines `CreateUserRequest` and `UpdateUserInfoRequest` with the same fields as `of_model::http::idp`. The service versions reject unknown fields; generated wire versions have different serialization attributes. These are separate Rust identities, not mere re-exports.
- **Actual setup/join contract split:** `setup/src/independent_operation.rs` owns `IndependentCreateIntent`, `IndependentCreateRequest`, and `IndependentOwnerPublic`; generated copies exist under `of_model::http` for several owners. `IndependentPeer` and `Challenge` have the same runtime-versus-wire split. `device-service/src/{independent_join,hosted_join}.rs` defines Join request/approval payloads also represented in generated wire types. Their validation/signing code is runtime behavior, but their serializable fields are still contract definitions outside the model. Leaving setup unchanged does not make this a strict single-source pass.
- **Other domain overlap:** `storage-sdk/src/sync.rs::SyncStatus` repeats the five status variants and camelCase serialization in `of_model::server::storage::resources::SyncStatus`. Its `SyncDirection` has only Bidirectional, while the HTTP enum also has Push/Pull; these are divergent shapes, not identical replacements. `storage-client::SyncStatus` is a different connection lifecycle (Synchronizing/Synced/Disconnected/Error), not the same session status.
- **Not domain duplicates:** `setup::LocalTls` holds a directory and `Arc<rustls::ServerConfig>` and performs certificate/file IO. `IndependentCreateHost`, control handles, registrar/loader callbacks, setup file helpers, and `TlsListener` are runtime implementation types. `device-server` middleware's `DeviceAuthorization` is an authenticated subject/application extractor, not the IdP OAuth device authorization response. Generated `device-client` operation `DeviceError` is an unknown-response wrapper, not the domain `DeviceError` enum. Same names alone do not prove duplicate domains.
- **Inside-model duplication:** generated per-owner `http` payloads and canonical `server`/domain payloads also remain separate definitions inside `of-model`. Central crate ownership prevents cross-crate source drift only in part; it does not prove one canonical definition per concept.

Only the database API gap required by ticket 02 is repaired: `of_model::error::DatabaseError` is a named re-export of the existing SQL `QueryError`, gated by `replica`. No new error or transaction protocol, domain reshuffle, or setup dependency is introduced. Consolidating the remaining payloads needs a separate model change with serialization and OpenAPI checks.
