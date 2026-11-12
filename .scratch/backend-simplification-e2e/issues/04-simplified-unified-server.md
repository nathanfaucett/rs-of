# Ticket 04: Simplified Unified Server Binary (REPLACE existing)

**Status: ready-for-agent**

**Blocked by:** 03

## What it delivers

**Replace** the existing `unified-server` binary with a simplified version. No `unified-server-simple` — just one binary that starts in seconds on an empty directory. Old binary and its setup flow are deleted.

## Scope

- **Replace** `unified-server` main binary (not add new)
- `UnifiedConfig` with only:
  - `listen_addr: SocketAddr`
  - `data_dir: PathBuf`
  - `tls: TlsConfig` (auto-generate self-signed)
  - `seed: Option<SeedConfig>` (optional test data)
- Startup sequence:
  1. `EmbeddedDatabase::open(data_dir)`
  2. Generate/load TLS cert at `data_dir/tls/`
  3. Build shared `UnifiedState { db, tls, config }`
  4. Mount four owner routers at `/idp`, `/management`, `/device`, `/storage`
  5. Serve on `listen_addr` with TLS
- **Delete** all `SetupHost`, `IndependentCreateHost`, `SetupAdmission` code
- **Remove** `setup` crate from unified-server dependencies
- CLI: `--data-dir`, `--port`, `--host`, `--no-tls` (for HTTP-only tests)
- Dynamic port allocation (port 0) with printed actual address
- **No backwards compatibility** — old config files, data layouts, CLI commands all invalid

## Acceptance Criteria

- `cargo run --bin unified-server -- --data-dir /tmp/test` succeeds on empty dir
- Creates only `/tmp/test/unified.redb` and `/tmp/test/tls/`
- All four owner endpoints respond (health checks at `/idp/health`, etc.)
- No `create`, `join`, `admission` commands exist
- TLS cert auto-generated and served
- `--no-tls` flag serves plain HTTP for simpler test environments
- Starts in <2 seconds on cold start
- Old `unified-server` CLI (`create`, `join`, `serve` with setup) completely removed
