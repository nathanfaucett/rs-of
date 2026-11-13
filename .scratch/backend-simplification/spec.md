# Spec: Backend Simplification for E2E Testing

## Problem Statement

The current unified-server startup is over-engineered for local development and E2E testing. It requires:
- Multiple independent data directories (`idp.setup-host`, `device.setup-host`, `storage.setup-host`, `endpoint.tls`)
- Complex TLS provisioning with local CA generation
- Multi-step admission/operation flow with capability codes
- Separate `IndependentCreateHost` instances per owner
- File-based state management across scattered directories under `of/data/unified/`

This complexity makes it impractical to spin up a real backend for E2E tests. Frontend teams resort to mocking, which creates a gap between test and production behavior.

## Solution

Replace the unified-server with a drastically simplified binary that:
- Starts in seconds on an empty directory with a single command
- Uses a single embedded redb database file (`unified.redb`)
- Has no setup ceremony — no admission, no capability codes, no CA generation
- Mounts all four owners (IDP, Management, Device, Storage) on one port
- Auto-generates self-signed TLS cert (or runs HTTP-only with `--no-tls`)
- Uses a single `of-model` crate as the sole source of truth for all types

The same binary works for:
- Local E2E tests (embedded DB, in-process)
- Independent deployments (external DB via `NetworkDatabase` impl)

## User Stories

1. As a **frontend developer**, I want to run `cargo run -p unified-server -- local --data-dir .data --port 1355 --origin http://localhost:5173` and have a working backend in <3s, so that I can run E2E tests against real HTTP endpoints.

2. As a **frontend developer**, I want Playwright tests to spawn the real unified-server in `beforeAll`, so that tests exercise actual API contracts without mocks.

3. As a **backend developer**, I want a single `of-model` crate containing all domain types, so that type drift between services is impossible and OpenAPI contracts have one source.

4. As a **backend developer**, I want to delete `SetupHost`, `IndependentCreateHost`, admission flow, capability codes, and all `of/data/unified/` directories, so that the codebase is simpler and easier to maintain.

5. As a **DevOps engineer**, I want the `setup` crate preserved ONLY for independent multi-node deployments, so that production paths are unchanged while local complexity is eliminated.

## Implementation Decisions

### 1. Single Model Crate: `of-model`

Create `of/crates/of-model/` as the **only** shared types crate. Consolidates:
- `idp-model` → all identity types (User, Credential, Application, Client, OAuth2, Roles, Permissions, Replication)
- `device-service` model types → Device, Enrollment, SelectionPolicy, ResourceSelection
- `storage-model` → StorageNamespace, ResourceCatalog, ResourceIdentity, ResourceKind, StorageSession, StorageEntry
- `management-model` → SyncSelection, ManagementSyncSelection
- `api` crate → Contracts, LocalTls, ServerConfig, setup file utilities

**Structure:**
```
of-model/
  Cargo.toml
  src/
    lib.rs              # re-exports all
    id.rs               # Id (UUID v7)
    user.rs             # User, emails, phones, credentials, MFA
    application.rs      # Application, Client, OAuth2 types
    permission.rs       # Role, Permission, UserRole, ApplicationUserRole
    device.rs           # Device, Enrollment, SelectionPolicy, ResourceSelection
    storage.rs          # Namespace, ResourceCatalog, ResourceIdentity, ResourceKind, Session, Entry, Request, Response
    replication.rs      # Sync, Admission, CopyRetirement, SyncSelection
    replica.rs          # single `up()` migration for ALL tables
  replica_migrations/
    0001_init.up.sql    # creates ALL tables in one DB
```

**Migration `0001_init.up.sql`**: Concatenate existing migrations in dependency order:
1. IdP tables (users, credentials, applications, clients, keys, OAuth2, roles, permissions, replica members)
2. Device tables (devices, enrollments, selection_policies, resource_selections)
3. Storage tables (namespaces, resources, databases, filesystems)
4. Management tables (sync_selections) — already in IdP migration

**Delete after migration**: `idp-model`, `storage-model`, `management-model` crates; `api` crate (contracts moved to `of-model`).

### 2. Database Abstraction in `of-model`

Define `Database` trait in `of-model/src/db.rs`:
```rust
pub trait Database: Send + Sync {
    type Txn: Transaction;
    fn txn(&self) -> Result<Self::Txn>;
    // Owner-specific method groups
    fn idp(&self) -> &dyn IdpDatabase;
    fn device(&self) -> &dyn DeviceDatabase;
    fn storage(&self) -> &dyn StorageDatabase;
    fn management(&self) -> &dyn ManagementDatabase;
}
```

Three implementations:
- `EmbeddedDatabase` (redb, single file) — unified server, E2E tests
- `NetworkDatabase` (PostgreSQL) — independent deployments
- `MemoryDatabase` (in-memory) — fast unit tests

Owner services depend on `Database` trait, not concrete types.

### 3. Embedded Database Crate: `embedded-db`

Create `of/crates/embedded-db/` implementing `Database` with redb:
- Single file: `<data_dir>/unified.redb`
- Embedded migration runner calls `of_model::replica::up()`
- No external dependencies, `no_std` compatible (with alloc)

### 4. Replace `unified-server` Binary Entirely

**Delete the existing binary and replace** (not add `unified-server-simple`):

**New `UnifiedConfig`:**
```rust
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub data_dir: PathBuf,           // single root
    pub tls: TlsConfig,              // embedded or disabled
    pub seed: Option<SeedConfig>,    // optional test data
}
```

**Startup sequence (no provisioning):**
1. Open/create `<data_dir>/unified.redb`
2. Generate/load TLS cert at `<data_dir>/tls/` (or skip with `--no-tls`)
3. Run `of_model::replica::up(&db)` — single migration
4. Mount owner routers at `/idp`, `/management`, `/device`, `/storage` sharing `Arc<Database>`
5. Serve on configured address

**CLI:**
```bash
cargo run -p unified-server -- local --data-dir .data --port 1355 --origin http://localhost:5173 [--no-tls]
```

**Auto-seeds:**
- Initial administrator: `admin@local.test` / `password`
- OIDC client for password-manager: `password-manager` with redirect `http://localhost:5174/callback`
- OIDC client for app: `app` with redirect `http://localhost:5173/callback`

### 5. Owner Services Accept Shared Database

Refactor each server's public entry point to accept optional `db: Arc<dyn Database>`:
- `idp_server::build_local_router(data_dir, port, origin, db)`
- `device_server::build_local_router(data_dir, port, idp_base, db)`
- `storage_server::build_local_runtime(data_dir, port, idp_base, device_base, db)`
- `management_server::build_local_router(data_dir, port, idp_base, storage_base, db)`

If `db` is `None`, open own database at `data_dir/{owner}.redb` (preserves independent deployment).

### 6. E2E Test Integration

**Playwright global setup** (`of/e2e/playwright.config.ts`):
```typescript
globalSetup: async () => {
  const backend = spawn('cargo', ['run', '-p', 'unified-server', '--', 'local', '--data-dir', '.scratch/e2e', '--port', '1355', '--origin', 'http://127.0.0.1:5173']);
  await waitForHealth('http://127.0.0.1:1355/idp/health');
}
```

**Frontend env (password-manager):**
```
PUBLIC_IDP_BASE_URL=http://127.0.0.1:1355/idp
PUBLIC_DEVICE_BASE_URL=http://127.0.0.1:1355/device
PUBLIC_STORAGE_BASE_URL=http://127.0.0.1:1355/storage
PUBLIC_OIDC_CLIENT_ID=password-manager
PUBLIC_OIDC_REDIRECT_URI=http://127.0.0.1:5174/callback
```

**Frontend env (app):**
```
PUBLIC_IDP_BASE_URL=http://127.0.0.1:1355/idp
PUBLIC_DEVICE_BASE_URL=http://127.0.0.1:1355/device
PUBLIC_STORAGE_BASE_URL=http://127.0.0.1:1355/storage
PUBLIC_URL=http://127.0.0.1:5173
```

### 7. Aggressive Cleanup (Delete, Don't Archive)

**Delete entirely:**
- `of/crates/unified-server/src/setup_host.rs` (240+ lines)
- `of/crates/unified-server/src/provisioning.rs`
- `of/crates/unified-server/src/bootstrap/`
- `of/crates/unified-server/src/independent_*.rs`
- `of/crates/unified-server/src/node_join*.rs`
- `of/data/unified/` (entire directory)
- `idp-model`, `storage-model`, `management-model` crates (after move)
- `api` crate (after move)
- `SetupHost`, `IndependentCreateHost`, `SetupAdmission`, `SetupCapability`, `LocalTls` (from unified path)
- All capability code files, admission flow code

**Preserve ONLY for independent deployments:**
- `of/crates/setup/` crate — unchanged, used by production multi-node deployments

## Testing Decisions

- **Highest seam**: HTTP API — Playwright tests hit real endpoints of running unified-server
- **Second seam**: `Database` trait — unit tests use `MemoryDatabase`, integration tests use `EmbeddedDatabase`
- **No mocks**: Delete all mock backend code in frontend tests
- **Test data**: Seed via `--seed` flag or direct DB writes in test `beforeEach`

## Out of Scope

- Changes to independent deployment model (multi-node, distributed)
- Changes to OAuth2/OIDC/device/storage protocols
- OpenAPI contract changes (generated from `of-model` types)
- Generated clients (`packages/*-client`) — they consume `of-model` types
- Frontend code changes (only env vars)
- Tauri/desktop integration
- Schema migration tooling (initial `0001_init` is fresh DB only)

## Further Notes

**Breaking changes policy**: This effort explicitly permits sweeping breaking changes. No backwards compatibility required:
- Delete old code — no migration path for existing unified-server data layouts
- No feature flags to preserve old behavior — replace, don't wrap
- No glue code to support past state — purge obsolete files/functions/blocks
- Config/schema changes — redesign from scratch; old configs invalid
- API changes — unified-server HTTP contracts can change; clients updated in same PR
- Crate structure — merge, split, rename crates freely

The `setup` crate is the **only** preserved component for independent deployments.
