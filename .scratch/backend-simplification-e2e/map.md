# Map: Backend Simplification for E2E Testing

## Notes

- Goal: Single-command unified-server startup on empty directory for E2E tests
- Core insight: Embedded DB (redb) + single model crate eliminates setup complexity
- Independent deployments unchanged — they use NetworkDatabase implementation
- Six vertical slices, each demoable
- **BREAKING CHANGES ALLOWED** — no backwards compat, no migration, delete old code

## Decisions-so-far

1. **Model crate**: Extend existing `model` → `of-model` as single source of truth
2. **Database**: redb for embedded (already in use), PostgreSQL for network
3. **TLS**: Auto-generated self-signed cert; `--no-tls` flag for HTTP-only tests
4. **Seam**: HTTP API is highest seam — tests hit real endpoints
5. **Test server**: Spawn in `beforeAll`, shutdown in `afterAll`, temp data dir per test run
6. **Replace, don't add**: Single `unified-server` binary replaced; no `unified-server-simple`
7. **Delete aggressively**: SetupHost, admission, capability codes, scattered data dirs — all gone
8. **setup crate preserved ONLY for independent deployments** — not a unified dependency

## Fog

- Exact `Database` trait surface — depends on owner service refactoring
- Seed data format — JSON? SQL? Direct API calls?
- Port allocation strategy — port 0 + printed address vs fixed port
- CI timeout budget for full stack test
- redb vs sqlite — stick with redb for now
