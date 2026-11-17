# Ticket 07: Port CI workflows and just recipes from old repos

Status: ready-for-agent

Blocked by: 01-copy-remaining-ofdb-crates, 02-rewrite-cargo-path-refs

## What it delivers

All lint, test, browser, wasm, and docker-publish jobs run against the merged `of` repo without external checkouts.

## Scope

1. **`of/.github/workflows/e2e.yml`**: remove the three `actions/checkout` steps for `rs-ofdb`, `rs-ofnet`, `rs-offs` and the ofdb package build step; build local `@ofsdk/sql` using the existing rust/wasm-pack versions
2. **Port `ofdb/.github/workflows/test.yml` jobs** into `of/.github/workflows/` (or extend `ci.yml`):
   - lint job with the remote-feature isolation greps: `cargo tree -p ofdb-sql --no-default-features --features remote | grep -E 'ofdb-(sql-engine|btree|sql-sync)|redb v|automerge v'` must be empty (same for ofdb-kv)
   - cargo-tests job with crap coverage (`just crap-summary`)
   - browser matrix jobs (`browser-orchestration-test`, `browser-check`, `browser-accept`, `browser-native`) driving the wasm-bindgen test harness
   - native regression job
3. **Port `ofnet/.github/workflows/ci.yml` wasm job** (`just build-wasm`, `just test-wasm`)
4. **Port `ofdb/.github/workflows/docker.yml`** (kv/sql server images) adapted to single-repo context
5. **Merge needed just recipes** from old repos into `of/justfile`:
   - ofdb: `browser-check`, `browser-accept`, `browser-measure`, `browser-native`, `browser-final`, `browser-orchestration-test`, `browser-native`
   - ofnet: `build-wasm`, `test-wasm`, `npm-install`, `net-build`, `net-example`, `npm-lint`
   - offs: any unique recipes
6. **Port supporting files** from old repos into `of/` counterparts:
   - `ofdb/tests/browser/` → `of/db/tests/browser/` (wasm-bindgen harness)
   - `ofdb/test-fixtures/` → `of/db/test-fixtures/`
   - `ofdb/scripts/*.sh` → `of/db/scripts/` or `of/scripts/`
   - `ofdb/Dockerfile.server`, `ofdb/compose.yaml` → `of/db/`
   - `ofnet/webdriver.json` → `of/net/`
   - `ofnet/tests/` if any → `of/net/tests/`

## Acceptance

- `gh workflow run` on `ci.yml` and `e2e.yml` passes (or at least parses and starts)
- `just browser-orchestration-test`, `just browser-check`, `just browser-accept`, `just build-wasm`, `just test-wasm` are defined and runnable
- The remote-feature isolation greps still pass for `ofdb-sql` and `ofdb-kv` in the merged workspace
- Docker publish workflow builds kv/sql images from `of/db/Dockerfile.server`
