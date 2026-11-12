# Unified Model - Map

## Notes

- Single model crate (`of-model`) replaces four: `idp-model`, `device-service` (inline), `storage-model`, `management-service` (reuses IdP)
- Single database file (`unified.redb`) replaces four: `idp.redb`, `devices.redb`, `management.redb` (same as IdP), `storage/`
- Local mode: HTTP, no TLS, no setup ceremony, auto-seeded admin + OIDC clients
- Production paths unchanged — independent deployments still work

## Decisions-so-far

1. **Consolidation approach**: New crate `of-model` with all types + single migration. Existing crates import from it.
2. **Shared DB pattern**: Each server's `build_*` gets optional `db` parameter. If `Some`, use shared; if `None`, open own.
3. **Local entry points**: New `build_local_router` functions deriving config from 3 inputs (`data_dir`, `port`, `origin`).
4. **CLI**: `local` subcommand on each server + unified server.
5. **E2E**: Playwright globalSetup spawns unified local backend.

## Fog

- Exact ordering of migrations in `0001_init.up.sql` (foreign key dependencies)
- Whether `storage-service` database runtime (`DatabaseRuntime`) can accept pre-opened `db::Database` or needs refactor
- How `device-service` replica migration validation interacts with shared DB (currently validates `devices` table columns)
- Seed data: which OIDC clients for which frontends (password-manager, app, others)
- Whether `management-service` LocalPermissions can work with shared DB (currently expects IdP tables in same DB — should work)
