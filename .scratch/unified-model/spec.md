# Unified Model Crate

## Summary

Extract a single shared model crate (`of-model`) containing all domain types and a unified database migration. Replace four separate model crates and four database files with one crate and one database. Enable local/E2E deployment via a simple HTTP server with no setup ceremony, while preserving production independent-deployment paths.

## Motivation

Current state:
- Four model crates: `idp-model`, `device-service` (model inline), `storage-model`, `management-service` (reuses `idp-model`)
- Four database files: `idp.redb`, `devices.redb`, `management.redb` (same as IdP), `storage/` (directory of databases)
- Complex setup ceremony: `setup-host/`, `setup-admission/`, TLS/CA, admission codes, capability files
- Unified server requires orchestrating four independent owner startups

Desired state:
- One model crate: `of-model` with all types
- One database file: `unified.redb` with all tables
- Local mode: `cargo run -p unified-server -- local --data-dir .data --port 1355 --origin http://localhost:5173` → HTTP, no ceremony
- Production: unchanged (independent deployments still work)

## Domain Vocabulary

Use existing glossary terms: **Installation**, **Node**, **Unified Host**, **User**, **Application**, **Principal**, **Device**, **Trusted Device**, **Bootstrap Service**, **Installation Setup**, **Setup Owner**, **Setup Capability**, **Device Setup**, **Reset Device**, **Initial Administrator**, **Trusted Peer**.

## Specification

### 1. New Crate: `of-model`

**Location**: `of/crates/of-model/`

**Structure**:
```
of-model/
  Cargo.toml
  src/
    lib.rs                    # re-exports all public types
    id.rs                     # Id (UUID v7), used everywhere
    user.rs                   # User, emails, phones, credentials, MFA
    application.rs            # Application, Client, OAuth2 types
    permission.rs             # Role, Permission, UserRole, ApplicationUserRole
    device.rs                 # Device, Enrollment, SelectionPolicy, ResourceSelection
    storage.rs                # Database, Filesystem, Resource, Namespace, Session
    replication.rs            # Sync, Admission, CopyRetirement
    replica.rs                # single `up()` migration for ALL tables
  replica_migrations/
    0001_init.up.sql          # creates ALL tables in one DB
```

**Types to consolidate** (from existing crates):

| From crate | Types | Target module |
|---|---|---|
| `idp-model` | `Id`, `User`, `UserEmail`, `UserPhoneNumber`, `Credential`, `Application`, `Client`, `Key`, `OAuth2AuthorizationCode`, `OAuth2RefreshToken`, `OAuth2UserConsent`, `Role`, `Permission`, `RolePermission`, `ApplicationUserRole`, `IdpReplicaMember`, `IdpReplicaSigner` | `user.rs`, `application.rs`, `permission.rs`, `replication.rs` |
| `device-service` | `Device`, `DeviceEnrollment`, `DeviceSelectionPolicy`, `DeviceResourceSelection`, `LocalDeviceCopyCleanup` | `device.rs` |
| `storage-model` | `StorageNamespace`, `ResourceCatalog`, `ResourceIdentity`, `ResourceKind`, `StorageSession`, `StorageEntry`, `StorageRequest`, `StorageResponse` | `storage.rs` |
| `management-service` | `SyncSelection`, `SyncSelectionRequest`, `SyncSelectionResponse`, `ManagementSyncSelection` | `replication.rs` |

**Migration** (`0001_init.up.sql`): Concatenate all existing migrations in dependency order:
1. IdP tables (users, credentials, applications, clients, keys, OAuth2, roles, permissions, replica members)
2. Device tables (devices, enrollments, selection_policies, resource_selections)
3. Storage tables (namespaces, resources, databases, filesystems)
4. Management tables (sync_selections) — already in IdP migration

### 2. Update Each Server to Accept Shared Database

**Pattern**: Add optional `db: db::Database` parameter to public `build_*` functions. If provided, use it; else open own database at `data_dir/{owner}.redb`.

**IdP Server** (`idp-server/src/runtime.rs`):
```rust
pub async fn build_oauth_runtime(
    config: &AppConfig,
    engine: db::Client,
) -> io::Result<IdpRuntime>  // unchanged

// NEW: local mode entry point
pub async fn build_local_router(
    data_dir: &Path,
    port: u16,
    origin: &str,
) -> io::Result<Router> {
    let db_path = data_dir.join("idp.redb");  // or unified.redb if shared
    let engine = open_local_identity_database(&db_path).await?;
    seed_local_idp(&engine, origin).await?;  // admin user + OIDC clients
    let config = LocalConfig { data_dir, port, origin }.derive_app_config();
    let runtime = build_oauth_runtime(&config, engine.client()).await?;
    Ok(runtime.router)
}
```

**Device Server** (`device-server/src/runtime.rs`):
```rust
pub async fn build_local_router(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    db: Option<db::Database>,  // shared or None
) -> Result<Router>
```

**Storage Server** (`storage-server/src/runtime.rs`):
```rust
pub fn build_local_runtime(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    device_base: &str,
    db: Option<db::Database>,
) -> io::Result<StorageRuntime>
```

**Management Server** (`management-server/src/runtime.rs`):
```rust
pub async fn build_local_router(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    storage_base: &str,
    db: Option<db::Database>,
) -> Result<Router>
```

### 3. Unified Server Local Compose

**New function** in `unified-server/src/runtime.rs`:
```rust
pub async fn compose_local_router(
    data_dir: &Path,
    port: u16,
    origin: &str,
) -> io::Result<Router> {
    // One database for all services
    let db_path = data_dir.join("unified.redb");
    let db = Arc::new(Database::open(&db_path).map_err(io::Error::other)?);
    of_model::replica::up(&db).await?;  // SINGLE migration

    let idp_base = format!("http://127.0.0.1:{port}/idp");
    let device_base = format!("http://127.0.0.1:{port}/device");
    let storage_base = format!("http://127.0.0.1:{port}/storage");
    let management_base = format!("http://127.0.0.1:{port}/management");

    // All services share the same DB
    let idp = idp_server::build_local_router(&data_dir.join("idp"), port, origin).await?;
    let device = device_server::build_local_router(&data_dir.join("device"), port, &idp_base, Some(db.clone())).await?;
    let storage = storage_server::build_local_runtime(&data_dir.join("storage"), port, &idp_base, &device_base, Some(db.clone())).await?;
    let management = management_server::build_local_router(&data_dir.join("management"), port, &idp_base, &storage_base, Some(db.clone())).await?;

    Ok(compose_router(idp, management, device, storage))
}
```

### 4. CLI: `--local` Subcommand

Each server's CLI (`cli::run`) adds a `local` subcommand:

```bash
# IdP only
cargo run -p idp-server --features cli --bin idp-server -- local --data-dir .data --port 1355 --origin http://localhost:5173

# Device only
cargo run -p device-server --features cli --bin device-server -- local --data-dir .data --port 1356 --idp-base http://127.0.0.1:1355/idp

# Unified (all 4)
cargo run -p unified-server --features cli --bin unified-server -- local --data-dir .data --port 1355 --origin http://localhost:5173
```

**Behavior**:
- HTTP only (no TLS, no `LocalTls`, no CA)
- No `IndependentSetupHost`, no `SetupAdmission`, no `setup-host/`, no `setup-admission/`
- Auto-creates/migrates database on first run
- Seeds initial administrator (`admin@local.test` / `password`) and OIDC clients for frontends
- Derives all config from 3 inputs: `data_dir`, `port`, `origin` (and peer bases for non-IdP)

### 5. E2E Integration

**Playwright globalSetup** (`of/e2e/playwright.config.ts`):
```typescript
globalSetup: async () => {
  const backend = spawn('cargo', ['run', '-p', 'unified-server', '--features', 'cli', '--bin', 'unified-server', '--', 'local', '--data-dir', '.scratch/e2e', '--port', '1355', '--origin', 'http://127.0.0.1:5173']);
  await waitForHealth('http://127.0.0.1:1355/idp/health');
}
```

**Frontend env** (password-manager):
```
PUBLIC_IDP_BASE_URL=http://127.0.0.1:1355/idp
PUBLIC_DEVICE_BASE_URL=http://127.0.0.1:1355/device
PUBLIC_STORAGE_BASE_URL=http://127.0.0.1:1355/storage
PUBLIC_OIDC_CLIENT_ID=password-manager
PUBLIC_OIDC_REDIRECT_URI=http://127.0.0.1:5174/callback
```

**Frontend env** (app):
```
PUBLIC_IDP_BASE_URL=http://127.0.0.1:1355/idp
PUBLIC_DEVICE_BASE_URL=http://127.0.0.1:1355/device
PUBLIC_STORAGE_BASE_URL=http://127.0.0.1:1355/storage
PUBLIC_URL=http://127.0.0.1:5173
```

### 6. Production Preservation

**No changes to production paths**:
- Existing `AppConfig` with all fields remains
- Existing `build_runtime`/`build_router` signatures unchanged
- Existing CLI `serve` subcommand unchanged
- Existing `IndependentSetupHost`, `LocalTls`, `SetupAdmission` unchanged
- Independent deployments (separate processes, separate DBs) work identically

**Migration for production**: None required. The new `of-model` crate is additive; services can opt into shared DB by passing `db` parameter.

## Seams

| Seam | Location | Purpose |
|---|---|---|
| `of-model::replica::up()` | `of-model/src/replica.rs` | Single migration entry point |
| `idp_server::build_local_router()` | `idp-server/src/runtime.rs` | Local HTTP entry point |
| `device_server::build_local_router()` | `device-server/src/runtime.rs` | Local HTTP entry point |
| `storage_server::build_local_runtime()` | `storage-server/src/runtime.rs` | Local HTTP entry point |
| `management_server::build_local_router()` | `management-server/src/runtime.rs` | Local HTTP entry point |
| `unified_server::compose_local_router()` | `unified-server/src/runtime.rs` | Composed local backend |
| CLI `local` subcommand | Each server's `cli.rs` | Developer-facing command |

**Single highest seam**: `unified_server::compose_local_router` — one call spins up the entire local backend.

## Acceptance Criteria

1. **`of-model` crate compiles** with all types re-exported, single `up()` migration runs without error
2. **`cargo run -p unified-server -- local --data-dir /tmp/test --port 1355 --origin http://localhost:5173`** starts in <3s, serves HTTP on port 1355
3. **`GET /idp/health` → 200**, **`GET /device/health` → 200**, **`GET /storage/health` → 200**, **`GET /management/health` → 200**
4. **IdP seeds admin user** (`admin@local.test` / `password`) and **password-manager OIDC client** (`password-manager`, redirect `http://localhost:5174/callback`)
5. **Password-manager app** can sign in via OIDC code flow against local backend, create vault, CRUD items
6. **App** can sign in, access device list, storage resources
7. **Production `serve` subcommand** still works unchanged (integration test: `tests/system_create.rs` passes)

## Non-Goals

- No changes to OpenAPI contracts (`of/openapi/*.json`)
- No changes to generated clients (`packages/*-client`)
- No changes to frontend code (only env vars)
- No changes to Tauri/desktop integration
- No schema migration tooling (initial `0001_init` is fresh DB only)

## Tickets

| # | Title | Type | Dependencies |
|---|---|---|---|
| 01 | Create `of-model` crate with consolidated types | task | — |
| 02 | Write unified migration `0001_init.up.sql` | task | 01 |
| 03 | Add `build_local_router` to `idp-server` + CLI `local` | task | 01, 02 |
| 04 | Add `build_local_router` to `device-server` + CLI `local` | task | 01, 02 |
| 05 | Add `build_local_runtime` to `storage-server` + CLI `local` | task | 01, 02 |
| 06 | Add `build_local_router` to `management-server` + CLI `local` | task | 01, 02 |
| 07 | Add `compose_local_router` to `unified-server` + CLI `local` | task | 03, 04, 05, 06 |
| 08 | E2E Playwright config + auth spec for password-manager | task | 07 |
| 09 | E2E Playwright config + signin/setup spec for app | task | 07 |
| 10 | Verify production `serve` unchanged (run system tests) | task | 07 |

Status: ready-for-agent
