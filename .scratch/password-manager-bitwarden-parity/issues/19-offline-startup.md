# 19: Start and reload the app offline

**What to build:** Reliable startup and reload of a previously loaded application without network access, including required database/WASM assets.

**Blocked by:** None (can start immediately).

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Inspect the existing application delivery path and reuse its supported offline mechanisms. Cache the production application assets and required database/WASM resources without caching decrypted vault content, keys, or sensitive responses.
- [x] After a successful initial load/install, start and reload with network access disabled, then create/select/unlock a test vault, edit a synthetic Login, lock, and reopen using real local persistence.
- [x] Do not depend on a running development server, a remote identity service, or a successful network request during the qualified offline flows. A first-ever visit without cached assets is not an offline guarantee.
- [x] Provide actionable states for missing/failed offline assets, denied local storage, quota limits, and browser limitations. Explain that browser-local storage can be evicted and is not a backup.
- [x] Ensure application cache updates do not delete vault storage or mix incompatible asset sets. Handle update failure without silently damaging the usable cached application.
- [x] Use repeatable real-browser checks against the built application, not mocked persistence alone. Use only synthetic test vaults; do not create replacement live comparison tabs.
- [x] Establish an asset/test convention that later feature tickets can extend. Final qualification covers the full typed-item release after those tickets land.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-19` in `.scratch/worktrees/password-manager-19`. Base `42391ef3c` equals the integration tip; ancestry verified. Tests use the spec-approved application/browser and real-persistence boundaries. No live comparison tabs or map edits.

## Answer

Implemented the current Login offline-startup baseline. Final full typed-item release qualification remains with the later feature tickets, as required above.

- `apps/password-manager/src/service-worker.ts`: SvelteKit production manifest precache, including the emitted database WASM. Static shell/assets only; no runtime sensitive-response cache. Version/base-path-specific caches, deployment-version checks before/after download, atomic install failure cleanup, waiting activation, and app-cache-only cleanup. Missing assets fail with 503 rather than mixing deployment versions. No IndexedDB reset or vault format changes.
- `src/lib/offline/OfflineStatus.svelte` and `offline.ts`: layout-owned readiness/status, local write probe, denied-storage/quota/browser guidance, missing-assets/install/update states, and eviction/backup warnings. Synchronous storage-write exceptions close the probe database and report failure.
- `scripts/check-offline.mjs` and `test:offline`: repeatable built-app Chromium checks in isolated temporary profiles/contexts with synthetic values, real SQL/WASM/IndexedDB/Web Crypto, bounded waits, and server/browser cleanup in `finally`. No live browser connection or Playwriter calls.
- `README.md`: deployment, asset extension, safe cache-only recovery, commands, checks, and qualification limits. The test config resolves the actual linked SQL package instead of assuming a sibling path relative to the worktree. Biome includes the browser harness and test config.

### Validation (2026-11-05)

Commands below ran from `apps/password-manager` unless noted. Direct Node commands avoid pnpm's symlinked task-state restriction.

- Red: the initial `node scripts/check-offline.mjs` failed waiting for offline readiness before implementation. The write-quota test also failed before synchronous write-error handling was added. The deployment-mismatch check failed (new worker incorrectly installed) before deployment-version checks were added.
- `node node_modules/vite/bin/vite.js build`: passed. The build includes `ofdb_wasm_bg.A3h-CwGP.wasm` (8,175,139 bytes). Existing SQL static/dynamic import warning remains.
- `node scripts/check-offline.mjs`: passed all 12 reported scenario checks:
  1. Offline reload, create vault, create/edit Login, lock, close/reopen tab, select/unlock, persisted values; zero server requests with the server stopped.
  2. Sensitive GET/POST responses outside the allowlist are not cached; recognizable synthetic credentials/items are absent from the app cache; WASM is precached.
  3. Failed WASM update retains the complete old shell offline and unrelated caches.
  4. Successful update waits for tabs to close, activates one complete new version, keeps unrelated caches, and preserves the source vault's edited Login.
  5. New browser process starts offline from the same temporary profile and unlocks persisted data with the server stopped and zero server requests.
  6. Deleted cached WASM reports missing assets, not readiness.
  7. Failed first WASM install reports actionable failure and removes the partial app cache.
  8. Storage-denial guidance (injected browser API failure).
  9. Write-quota guidance (injected synchronous IndexedDB transaction failure).
  10. Unsupported-browser guidance (injected missing WebAssembly).
  11. First-ever offline visit fails as documented.
  12. A deployment version change after asset download rejects the install, removes its cache, and retains the previous complete version.
- `node node_modules/vitest/vitest.mjs run`: 5 files, 23 tests passed, including real-persistence vault tests. Initial run failed because the worktree-relative SQL allowlist was wrong; resolved by deriving the installed package location.
- `node node_modules/@sveltejs/kit/svelte-kit.js sync` and `node node_modules/svelte-check/bin/svelte-check --tsconfig ./tsconfig.json`: passed; 0 errors, 0 warnings.
- Biome check on all new/changed offline code/config/test files: 8 files passed, no fixes needed. Svelte autofixer on the new component and layout: no issues or suggestions. `git diff --check`: passed.
- Root `cargo hack test --feature-powerset --all-targets`: blocked before tests; Cargo expects missing `.scratch/worktrees/ofdb/crates/sql/Cargo.toml` through the workspace's sibling dependency paths. No Rust code changed.
- Root `just crap`: blocked in `sdk-build`; pnpm refuses the symlinked worktree `node_modules` task-state path (`ERR_PNPM_UNSAFE_TASK_RUN_STATE_PATH`). No CRAP result is claimed.

### Limits

Qualified Chromium, the built root-path app, and the current Login workflow only. Storage faults are injected; actual disk exhaustion, private-mode/browser eviction policies, other browser engines, mobile browsers, non-root deployments, backup/security qualification, and future typed-item release flows are not claimed. A completely evicted shell cannot display an in-app offline warning. Cache-only recovery must never clear site data or IndexedDB. Existing vault authentication/identity/backup limits belong to their own tickets. No route `+page` files or map files were changed.
