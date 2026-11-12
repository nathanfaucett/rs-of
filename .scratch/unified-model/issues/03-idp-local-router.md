# Ticket 03: Add `build_local_router` to `idp-server` + CLI `local`

**Status: ready-for-agent**

**Blocked by:** 01, 02

## What it delivers

`cargo run -p idp-server --features cli --bin idp-server -- local --data-dir .data --port 1355 --origin http://localhost:5173` starts an HTTP IdP server (no TLS, no setup ceremony) that serves `/idp/*` routes, seeds admin user + OIDC clients, and responds to health checks.

## Scope

1. Add `LocalConfig` struct in `idp-server/src/runtime.rs`:
   ```rust
   pub struct LocalConfig {
       pub data_dir: PathBuf,
       pub port: u16,
       pub origin: String,
   }
   impl LocalConfig {
       pub fn derive_app_config(&self) -> AppConfig { ... } // HTTP URLs, fast password config
   }
   ```
2. Add `build_local_router(data_dir, port, origin) -> io::Result<Router>`:
   - Opens/creates `data_dir/idp.redb` (or shared DB if passed)
   - Runs `of_model::replica::up()` (or `idp_model::replica::up()` during transition)
   - Seeds: admin user (`admin@local.test` / `password`), password-manager OIDC client (`password-manager`, redirect `http://localhost:5174/callback`), app OIDC client
   - Calls existing `build_oauth_runtime` with derived config
   - Returns `runtime.router`
3. Add `local` subcommand to `idp-server/src/cli.rs`:
   - Args: `--data-dir`, `--port`, `--origin`
   - Calls `build_local_router`, serves with `axum::serve` on HTTP
4. Export `build_local_router` from `idp-server/src/lib.rs`

## Acceptance criteria

- `cargo run -p idp-server --features cli --bin idp-server -- local --data-dir /tmp/test --port 1355 --origin http://localhost:5173` starts in <3s
- `GET http://127.0.0.1:1355/idp/health` → 200
- `GET http://127.0.0.1:1355/idp/.well-known/openid-configuration` → valid OIDC metadata
- Admin user exists and can authenticate via password grant
- OIDC clients registered and usable for code flow
