# Ticket 03: Refactor Owner Services for Shared Database

**Status: ready-for-agent**

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
