# Spec: Backend Simplification for E2E Testing

## Problem Statement

The current unified-server startup and setup process is over-engineered for local development and E2E testing. It requires:

- Multiple independent data directories (`idp.setup-host`, `device.setup-host`, `storage.setup-host`, `endpoint.tls`)
- Complex TLS provisioning with local CA generation
- Multi-step admission/operation flow with capability codes
- Separate `IndependentCreateHost` instances per owner
- File-based state management across scattered directories

This complexity makes it impractical to spin up a real backend for E2E tests. Frontend teams resort to mocking, which creates a gap between test and production behavior.

## Goal

Simplify the unified-server so it can be started with a **single command** against an **empty directory**, using an **embedded database** (redb/sqlite), with **no manual setup steps**. The same binary should work for:

- Local E2E tests (embedded DB, in-process)
- Independent deployments (external DB, networked)

## Domain Vocabulary

From `GLOSSARY.md`:

- **Owner**: IDP, Management, Device, Storage — the four logical services
- **Unified Host**: Single binary composing all four owners
- **Setup/Provisioning**: First-time initialization creating identity, keys, admission state
- **Embedded DB**: redb or sqlite database file within the data directory
- **Model Crate**: Single source of truth for all domain types (contracts, requests, responses)

## Non-Goals

- Change the independent deployment model (multi-node, distributed)
- Remove the ability to run owners as separate services
- Alter the OAuth2/OIDC/device/storage protocols
- Replace the existing `setup` crate — it remains for independent deployments

## Breaking Changes Policy

**This effort explicitly permits sweeping breaking changes.** No backwards compatibility required:

- **Delete old code** — `SetupHost`, `IndependentCreateHost`, admission flow, capability codes, scattered setup-host directories
- **No migration path** for existing unified-server data layouts — fresh start on empty directory
- **No feature flags** to preserve old behavior — replace, don't wrap
- **No glue code** to support past state — purge obsolete files/functions/blocks
- **Config/schema changes** — redesign from scratch; old configs invalid
- **API changes** — unified-server HTTP contracts can change; clients updated in same PR
- **Crate structure** — merge, split, rename crates freely (e.g., `model` → `of-model`)

The `setup` crate is the **only** preserved component — it continues serving independent deployments unchanged. Everything else in the unified codepath is fair game for deletion and redesign.

## Architecture

### Single Model Crate

Create `of-model` (or extend existing `model` crate) as the **sole** definition of:

- All request/response types (OpenAPI contracts)
- Domain entities (User, Device, Vault, Credential, etc.)
- Error types
- Configuration structs

All crates (`idp-server`, `management-server`, `device-server`, `storage-server`, `unified-server`, clients) depend **only** on `of-model` for shared types.

```
of-model (no_std, serde)
  ├─ contracts/        # OpenAPI operation definitions
  ├─ domain/           # Core entities
  ├─ config/           # AppConfig, ServerConfig shared shapes
  └─ error/            # Unified error types
```

### Unified Server — Simplified Runtime

```
unified-server (binary)
  ├─ Embedded DB (redb) at <data_dir>/unified.redb
  ├─ Single TLS config (self-signed cert generated on first run)
  ├─ Four owner routers mounted at /idp, /management, /device, /storage
  ├─ Shared runtime state (Arc<UnifiedState>)
  └─ No setup-host, no admission flow, no capability codes
```

**Startup sequence:**

1. Open/create `<data_dir>/unified.redb`
2. Generate/load TLS cert at `<data_dir>/tls/`
3. Initialize schema (migrations embedded)
4. Mount owner routers with shared state
5. Serve on configured address

**No provisioning step.** Owners start in "ready" state. Test data seeded via API or direct DB access.

### Configuration — Minimal

```rust
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub data_dir: PathBuf,           // single root
    pub tls: TlsConfig,              // embedded or external
    pub seed: Option<SeedConfig>,    // optional test data
}
```

No per-owner `data_dir`, no `api_public_uri` computation, no prefix validation.

### Database Strategy

| Deployment Mode      | Database                         | Use Case                      |
| -------------------- | -------------------------------- | ----------------------------- |
| Unified (E2E, local) | Embedded redb (single file)      | Zero-config, fast, in-process |
| Independent          | External PostgreSQL/redb network | Production, multi-node        |

The **same owner implementations** work in both modes — they receive a `Database` trait object. Unified server provides `EmbeddedDatabase`; independent servers provide `NetworkDatabase`.

## Seams for Testing

### Highest Seam: HTTP API

E2E tests hit the **real HTTP endpoints** of the running unified-server. No mocking.

```
Test → HTTP → unified-server (embedded DB) → Response
```

### Second Seam: Database Trait

For unit/integration tests needing isolated state:

```rust
trait Database: Send + Sync {
    fn txn(&self) -> Transaction;
    // owner-specific methods
}
```

Implementations:

- `EmbeddedDatabase` — redb file (unified, tests)
- `NetworkDatabase` — TCP to PostgreSQL (independent)
- `MemoryDatabase` — in-memory (fast unit tests)

Owner services depend on `Database`, not concrete types.

## Migration Path

### Phase 1: Model Crate Extraction

1. Move all shared types from `api`, `idp-model`, `storage-model`, `management-model` → `of-model`
2. Update all crates to depend on `of-model`
3. Remove duplicate type definitions

### Phase 2: Database Abstraction

1. Define `Database` trait in `of-model` (or `of-db` crate)
2. Implement `EmbeddedDatabase` (redb)
3. Implement `NetworkDatabase` (PostgreSQL client)
4. Refactor owner services to accept `Database` trait

### Phase 3: Simplified Unified Server

1. New `unified-server-simple` binary (or replace existing)
2. Single `UnifiedConfig` with `data_dir` only
3. Embedded DB initialization on startup
4. Direct router composition (no `SetupHost`, no `IndependentCreateHost`)
5. Self-signed TLS cert auto-generation

### Phase 4: E2E Test Integration

1. Add `unified-server` as test dependency in frontend apps
2. Vitest/Playwright config: `beforeAll` spawns server, `afterAll` kills it
3. Tests use real HTTP client against `https://localhost:PORT`
4. Optional: test data seeding via admin API or direct DB

### Phase 5: Cleanup

1. Remove `setup/` crate from unified-server dependencies
2. Remove `data/unified/*` directories from repo
3. Delete `SetupHost`, `IndependentCreateHost`, admission flow code
4. Archive old provisioning tests

## Acceptance Criteria

1. **Zero-config start**: `cargo run --bin unified-server -- --data-dir /tmp/test` works on empty directory
2. **Single data file**: Only `/tmp/test/unified.redb` and `/tmp/test/tls/` created
3. **E2E test runs**: Password-manager Playwright tests pass against real backend
4. **No setup commands**: No `create`, `join`, `admission` steps required
5. **Independent deployments unchanged**: Existing multi-node deployment still works via `NetworkDatabase`

## Risks & Mitigations

| Risk                                          | Mitigation                                                             |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| Embedded DB doesn't match production behavior | Use same redb crate; schema identical; only concurrency model differs  |
| TLS cert generation fails in CI               | Use `rcgen` with fixed test CA; or `--no-tls` flag for pure HTTP tests |
| Owner services assume independent runtime     | Refactor services to be runtime-agnostic (already mostly true)         |
| Migration breaks existing deployments         | Phase 1-2 are additive; Phase 3 introduces new binary alongside old    |

## Open Questions

1. **redb vs sqlite**: redb is already used; sqlite has better tooling. Decision: stick with redb for consistency.
2. **TLS in tests**: Real HTTPS needed for cookie/secure contexts. Generate self-signed cert on first run, trust in test setup.
3. **Seed data**: How to populate test users/vaults? Options: (a) admin API calls in test `beforeEach`, (b) direct DB writes, (c) `--seed` flag with JSON file.
4. **Port allocation**: Use port 0 for dynamic allocation; return actual port to test runner.

## References

- `of/crates/unified-server/src/setup_host.rs` — current complex setup flow
- `of/crates/setup/src/` — independent setup crate (to remain for deployments)
- `of/crates/model/src/` — current minimal model crate (to expand)
- `of/data/unified/` — current scattered data layout (to eliminate)
