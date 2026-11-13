# Ticket 04: Replace Unified Server Binary

**Status: ready-for-agent**

**Blocked by:** 03

## What it delivers

A completely new `unified-server` binary that starts in seconds on an empty directory. Replaces the existing binary entirely (not a new `unified-server-simple`). No setup-host, no admission flow, no capability codes, no CA generation.

## Scope

### Delete from `unified-server`:
- `setup_host.rs` (entire file — 240+ lines of admission/operation/capability code)
- `provisioning.rs`
- `bootstrap/` directory
- `independent_*.rs` files
- `node_join*.rs` files
- All `SetupHost`, `IndependentCreateHost`, `SetupAdmission`, `SetupCapability`, `LocalTls` usage

### New `UnifiedConfig` (minimal):
```rust
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub tls: TlsConfig,              // AutoGenerate | Disabled | External
    pub seed: Option<SeedConfig>,
}

pub enum TlsConfig {
    AutoGenerate,  // self-signed cert at <data_dir>/tls/
    Disabled,      // HTTP only (--no-tls flag)
    External { cert_path: PathBuf, key_path: PathBuf },
}
```

### New `compose_local_router` in `unified-server/src/runtime.rs`:
```rust
pub async fn compose_local_router(config: &UnifiedConfig) -> io::Result<Router> {
    // 1. Open shared database
    let db_path = config.data_dir.join("unified.redb");
    let db = Arc::new(EmbeddedDatabase::open(&db_path)?);
    of_model::replica::up(&*db).await?;

    // 2. TLS
    let tls = match &config.tls {
        TlsConfig::AutoGenerate => Some(load_or_generate_tls(&config.data_dir.join("tls"))?),
        TlsConfig::Disabled => None,
        TlsConfig::External { cert_path, key_path } => Some(load_external_tls(cert_path, key_path)?),
    };

    // 3. Base URLs (all on same port, different paths)
    let base = format!("http{}://127.0.0.1:{}/", if tls.is_some() { "s" } else { "" }, config.listen_addr.port());
    let idp_base = format!("{}idp", base);
    let device_base = format!("{}device", base);
    let storage_base = format!("{}storage", base);
    let management_base = format!("{}management", base);

    // 4. Mount all owners sharing the SAME database
    let idp = idp_server::build_local_router(&config.data_dir.join("idp"), config.listen_addr.port(), &idp_base, Some(db.clone())).await?;
    let device = device_server::build_local_router(&config.data_dir.join("device"), config.listen_addr.port(), &idp_base, Some(db.clone())).await?;
    let storage = storage_server::build_local_runtime(&config.data_dir.join("storage"), config.listen_addr.port(), &idp_base, &device_base, Some(db.clone())).await?;
    let management = management_server::build_local_router(&config.data_dir.join("management"), config.listen_addr.port(), &idp_base, &storage_base, Some(db.clone())).await?;

    // 5. Compose
    Ok(compose_router(idp, management, device, storage, tls))
}
```

### CLI `local` subcommand:
```bash
cargo run -p unified-server -- local --data-dir .data --port 1355 --origin http://localhost:5173 [--no-tls]
```

**Behavior:**
- HTTP or HTTPS (auto-generated self-signed cert)
- No `IndependentSetupHost`, no `SetupAdmission`, no `setup-host/`, no `setup-admission/`
- Auto-creates/migrates database on first run
- Seeds admin user + OIDC clients for frontends
- Derives all config from 3 inputs: `data_dir`, `port`, `origin`

## Acceptance Criteria

- `cargo run -p unified-server -- local --data-dir /tmp/test --port 1355 --origin http://localhost:5173` starts in <3s
- Serves HTTP on port 1355 (or HTTPS with `--no-tls` omitted)
- `GET /idp/health` → 200, `GET /device/health` → 200, `GET /storage/health` → 200, `GET /management/health` → 200
- Only `/tmp/test/unified.redb` and `/tmp/test/tls/` created
- Admin user `admin@local.test` / `password` exists and can sign in
- OIDC client `password-manager` with redirect `http://localhost:5174/callback` registered
- Old `serve` subcommand for independent deployments still works (if kept)
