# Ticket 03: Refactor Owner Services for Shared Database

**Status: claimed**

**Owner:** backend-simplification-owners (`.scratch/worktrees/owners`)

Complexity: medium; risk: shared storage catalogs and local authentication; priority: blocks ticket 04. Tests use the requested public constructor/schema/seed boundaries.

**Blocked by:** 01, 02

## What it delivers

Each owner service (`idp-server`, `device-server`, `storage-server`, `management-server`) accepts an optional shared `Database` trait object. When provided, all owners use the same database instance (enabling unified-server). When `None`, each opens its own database file (preserving independent deployment).

## Scope

### IdP Server (`idp-server/src/runtime.rs`)

```rust
// NEW: local mode entry point with shared DB
pub async fn build_local_router(
    data_dir: &Path,
    port: u16,
    origin: &str,
    db: Option<Arc<dyn Database>>,
) -> io::Result<Router>
```

- If `db` is `Some`, use it; else open `data_dir/idp.redb`
- Seed initial admin user (`admin@local.test` / `password`) and OIDC clients for frontends
- Derive config from `data_dir`, `port`, `origin` only

### Device Server (`device-server/src/runtime.rs`)

```rust
// NEW: local mode entry point with shared DB
pub async fn build_local_router(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    db: Option<Arc<dyn Database>>,
) -> Result<Router>
```

### Storage Server (`storage-server/src/runtime.rs`)

```rust
// NEW: local mode entry point with shared DB
pub fn build_local_runtime(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    device_base: &str,
    db: Option<Arc<dyn Database>>,
) -> io::Result<StorageRuntime>
```

### Management Server (`management-server/src/runtime.rs`)

```rust
// NEW: local mode entry point with shared DB
pub async fn build_local_router(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    storage_base: &str,
    db: Option<Arc<dyn Database>>,
) -> Result<Router>
```

**Preserve existing signatures** for independent deployments (no breaking changes to `serve` path).

## Acceptance Criteria

- Each server compiles with optional `db` parameter
- `cargo test -p idp-server`, etc. pass
- Independent deployment (`serve` subcommand) still works unchanged
- When `db` is `Some`, all owners share the same database instance

## Answer — 2026-11-12

Status remains **claimed**, not resolved. The implementation is ready for review, but the owner compilation and independent-runtime acceptance checks are blocked by existing errors. Tickets 01 and 02 also remain claimed.

### Delivered

- Added and exported all four requested local entrypoints with `Option<Arc<dyn of_model::db::Database>>`. Shared mode uses the supplied query client, with no private owner-file fallback. `None` opens `idp.redb`, `device.redb`, `storage.redb`, or `management.redb` and initializes the unified schema. The small shared helper is `embedded_db::local_client`.
- Local routers use the normal owner routes at `/idp`, `/device`, `/management`, and `/storage`. They do not create setup hosts, admission operations, capabilities, TLS authorities, or a mesh endpoint. Existing independent serving signatures and paths remain. No setup crate files changed.
- IdP seeds `admin@local.test` / `password`, explicit initial administrator grants, both public frontend clients and callbacks, and three restricted local service clients for IdP validation/user lookup. Stable identities and existing repositories make repeat initialization safe. The normal signer-trust repository supplies signing trust without a setup operation.
- Added a private-file local backend to the existing concrete private-key repository. Local IdP startup does not require an OS keyring. Independent keyring construction remains unchanged. Tests cover persistence, namespace isolation, deletion, and rejection of public/symlink directories on Unix.
- Seed tests exposed incomplete user-row writes/projections and incorrect INTEGER-to-bool decoding for `blocked`. Fixed those mappings in `idp-service`; creation, lookup, repeat seed, password verification, and signed service-token issuance now pass through the real services.
- Storage's local database runtime now uses the actual shared SQL `namespaces`, `resources`, and `databases` tables for its database catalog, tombstones, initialization state, and cleanup inventory. Catalog edits use one transaction. Existing namespace rows are reused; cross-namespace resource collisions are rejected. Independent Storage still selects its existing file catalog.
- Resource database content, KV sidecars, physical-copy journals, and offs filesystem catalogs/metadata/content remain files. The filesystem API has no SQL-client catalog injection boundary; no unused SQL mirror was added. A stable public filesystem peer label is retained in `storage-local.peer`, without binding an endpoint. See [local owner details](../owner-services.md) for the exact resource-file and backup requirements.

### TDD and focused validation

- Invoked the `tdd` skill. Used the constructor/schema/seed boundaries requested by the user. The local-client test first failed on the missing helper. The private-key test first failed on missing `new_local`. Catalog and seed tests then found real executor and row-mapping errors before passing. Constructor tests exist but cannot reach test execution through the broken dependencies; this is not a claimed constructor red/green pass.
- `cargo test -p embedded-db --test local --target-dir ../model/target`: **3 passed**. Checks shared writes through all owner selections without owner files, each named native file, unified schema, reopen, rejected owner names, and invalid-file preservation.
- `cargo test -p idp-service --features replica --test local_private_keys --target-dir ../model/target`: **2 passed**.
- `cargo test --manifest-path .scratch/backend-simplification/owner-validation/Cargo.toml --target-dir ../model/target --lib`: **15 passed**. Runs 12 existing production database catalog/runtime tests and 3 new catalog/runtime/seed tests against the actual source modules. Checks SQL catalog sharing, existing namespaces, owner isolation, collision rollback, lifecycle, real resource files and reopen, repeat seed, password verification, frontend callbacks, and signed service tokens.
- The isolated validation crate uses real SQL, IdP, Management, and file-key implementations. Its minimal RouterState fixture is not the server RouterState. It deliberately excludes the broken Device/offs dependencies and **does not prove that the HTTP constructors compile**. See its [README](../owner-validation/README.md).
- `cargo clippy -p embedded-db -p idp-service --features idp-service/replica --lib --no-deps --target-dir ../model/target`: passed with one existing redundant-closure warning in `user_repo.rs`; not warning-free.
- Ran targeted rustfmt on changed modules with `skip_children=true` to avoid formatting unrelated broken modules. `git diff --check` passed. Setup diff is empty.

### Blocked acceptance and limits

- `cargo test -p idp-server --no-default-features --test local --target-dir ../model/target`: initially timed out during dependency compilation; rerun failed on the same **62 Device replication errors** recorded by ticket 01. It did not compile or execute the IdP constructor test.
- `cargo test -p idp-server -p device-server -p management-server -p storage-server --test local --target-dir ../model/target`: failed on **4 existing offs `KvSnapshot` Serialize/Deserialize errors**. No owner constructor test ran.
- Required `cargo hack test --feature-powerset --workspace --all-targets --target-dir ../model/target`: failed on of-model run **3/202**, at `src/idp/replica.rs:765`, which implements removed `ofdb_sql::SyncTransport`. The alloc-only model run passed its 17 unit and 3 contract tests first.
- Required `just crap`: timed out after **60 seconds** during coverage compilation. No CRAP score or coverage pass was obtained.
- Live HTTP frontend flows, independent `serve`, filesystem SQL sharing, Windows ACLs, and live network-database catalog calls were not qualified. Local seed timing is not a startup-performance pass. The existing IdP router delimiter issue noted in ticket 01 remains outside this change.
- SQL catalog calls retain the existing synchronous Storage API and use scoped SQL worker threads. This is validated on local engines, not on a same-thread loopback network service.

Work is confined to `.scratch/worktrees/owners`, branch `backend-simplification-owners`, based on integration commit `15282eaf1`. The branch will merge the current integration tip into itself before handoff. It must not be merged into integration by this agent.
