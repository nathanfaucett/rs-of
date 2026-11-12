# Ticket 05: E2E Test Integration (Real Backend, No Mocks)

**Status: ready-for-agent**

**Blocked by:** 04

## What it delivers

Frontend apps (password-manager, app) run Playwright/Vitest browser tests against a **real** unified-server backend started programmatically. **Delete all mock backends** — tests hit real HTTP endpoints.

## Scope

- Add test utility in `of/crates/unified-server/test-support`:
  - `TestServer::spawn(config)` — starts server in background, returns base URL
  - `TestServer::shutdown()` — graceful stop
  - Auto-assigns free port, generates temp data dir
- Password-manager `vitest.config.ts`:
  - `beforeAll`: spawn `TestServer`, store URL in `testContext`
  - `afterAll`: shutdown server
  - Tests use `fetch`/`api-client` against real backend
- **Delete** any existing mock backend utilities in frontend tests
- Optional: `--seed` flag loads test fixtures (users, vaults, devices)
- CI integration: GitHub Actions runs `cargo test --workspace` + `pnpm test`
- Document pattern for other frontend apps

## Acceptance Criteria

- `pnpm test` in password-manager starts real backend, runs Playwright tests, shuts down
- Tests pass against real HTTP endpoints (**zero mocks**)
- Test isolation: each test file gets fresh server or fresh DB (configurable)
- CI runs full stack test in <5 minutes
- No manual backend startup required
- Test server cleanup on failure (no orphan processes)
- Mock backend code removed from password-manager and app test suites
