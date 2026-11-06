# 20: Qualify the complete offline release

**What to build:** Repeatable release evidence that the complete core personal-vault feature set works offline and preserves data through migration, recovery, and failures.

**Blocked by:** 04 — Export and restore complete encrypted backups; 11 — Search, filter, and sort unlocked items; 12 — Organize items in encrypted folders; 13 — Recover deleted items through Trash; 14 — Generate passwords and passphrases; 15 — Generate TOTP codes locally; 16 — Enforce manual and automatic lock; 17 — Preview and atomically import Bitwarden JSON; 18 — Export the supported Bitwarden JSON subset; 19 — Start and reload the app offline.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Run the full application test/build checks and a repeatable real-browser suite with synthetic vaults, real persistence, production assets, and disabled network access. Do not replace evidence with mocked UI results.
- [x] Exercise all five item types through create/view/edit/cancel/save/reopen, combined search/filter/sort, folders, Trash restore/permanent deletion, generator, TOTP, and protected copy/history actions.
- [x] Verify existing-vault format transitions and complete encrypted backup restore in a clean profile without the source environment. Compare all supported logical data, including folders, Trash, history, and optional fields.
- [x] Round-trip synthetic Bitwarden JSON through export/import and compare the documented supported subset. Confirm exclusions, duplicate handling, plaintext warnings, and no silent field loss.
- [x] Test wrong passwords for empty/populated vaults, vault-name collisions, damaged metadata/backups/imports, storage denial/quota errors, failed and uncertain writes, cancelled operations, and atomic import rollback.
- [x] Test offline startup/reload with cached database assets, cache-update failure, inactivity deadlines after tab suspension, stale handles, pending writes, and late callbacks after lock. No core operation may require a successful network request.
- [x] Verify keyboard/focus/error behavior, masked sensitive values, denied clipboard access, no persistent plaintext search/index/generated history, and no sensitive logs or URLs. Document browser memory/clipboard/storage limits honestly.
- [x] Fix integration failures caused by this effort, using the existing owner boundaries rather than wrappers. If storage-library code changed, run the affected Rust repository's required validation as well.
- [x] Record exact commands, environment, pass/fail outcomes, supported reference subset, and remaining limits in stable named qualification/operations documentation. Do not claim full Bitwarden parity, online-feature offline support, or checks that were not run.

## Answer

The complete synthetic Chromium core qualification passes. Stable [qualification and operations](../../../docs/password-manager-qualification.md) records exact commands, environment, evidence boundaries, fault injections, safe operations and remaining limits. No full parity, selected reference-client acceptance, security-audit or production-readiness claim is made.

### Delivered

- Extended the existing `check-offline.mjs` and `check-json-export.mjs` entry points. `test:offline` now builds once and runs both sequentially. Existing core/tool/cache/update/cold-process/recovery checks remain; no duplicate server/browser framework or product storage path was added.
- All five built editors exercise create/view/edit/cancel/save/reopen. Card now also saves a change and checks it after reopen. Existing Login/Note/Identity/SSH, combined search/filter/sort, folders, Trash, generator, TOTP and protected/history checks remain.
- Added `check-upgrade.mjs`: independently constructs a historical authenticated v1 wire fixture, restores it through the supported UI offline, rejects a wrong unlock password, performs actual legacy-to-typed conversion, compares the complete known logical active/Trash records, preserves nested URI/custom/history/date data and exact retained source recovery, then reloads/reopens. This is a recovery-format upgrade fixture, not execution of an old application binary. Existing API tests separately cover original v1 SQL history/registration, failed staging, unchanged retained source and stale handles.
- Added `check-json-roundtrip.mjs` through the existing export entry point. Full independent static wire/local fixtures cover every supported field for all five kinds, seven URI states, ordered custom variants/history, Unicode/multiline/leading zeroes and optional values. Actual confirmed UI export is compared with the independent wire oracle before fresh-destination preview/import. Fresh/remapped IDs, empty folders, counted Trash exclusion, source preservation and reopen are checked. Corrupt/unknown/type files block the complete batch; cancel and unconfirmed duplicates write nothing; explicit duplicate consent creates two complete sets, not an overwrite.
- Added meaningful built import quota rejection using the proven IndexedDB `records` write boundary. Confirm injection reached, fixed no-success feedback, exact unchanged destination, then successful fresh-preview retry. Full real-persistence API tests cover quota/lost/rejected/uncertain publication, one atomic batch, concurrency, pending writes and late lock results; these were reused, not duplicated with weaker UI mocks.
- Closed the source browser context before clean offline recovery. Compare exact authenticated/encrypted payload plus the complete decrypted logical snapshot, folders including empty, Trash, history, provenance, optional data and encrypted settings. Independent supported-wire output also matches after restore.
- Added built denied-clipboard feedback and simulated wall-clock suspension/focus-resume expiry with a dirty draft. Committed data remains unchanged after authenticated reopen. No actual OS-lock, guaranteed clipboard clearing, physical disk exhaustion or JavaScript zeroization is claimed.
- Updated stable import/export/item docs and app README to link current qualification and remove stale Login-only/incomplete-backup claims. Parent spec and shared map remain unchanged for the merger. No runtime source, storage library, migration, dependency version or lockfile changed.

### Validation

All browser/build/check commands used `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`, from this worktree's `apps/password-manager`. Every Vitest command used `--maxWorkers=1 --no-file-parallelism`.

- `node node_modules/vitest/vitest.mjs run --maxWorkers=1 --no-file-parallelism`: **33 files / 388 tests passed**, 519.40 seconds, real Chromium SQL/WASM/IndexedDB/Web Crypto.
- `node node_modules/@sveltejs/kit/svelte-kit.js sync` and `node node_modules/svelte-check/bin/svelte-check --tsconfig ./tsconfig.json`: **0 errors / 0 warnings**.
- `node node_modules/vite/bin/vite.js build && node scripts/check-offline.mjs && node scripts/check-json-export.mjs`: **passed** sequentially under one lock on the final scripts. No assertions removed or timeouts relaxed.
- Targeted `biome check` on all six touched browser scripts: **passed** after formatting. `git diff --check` and immutable-source checks are recorded at final handoff.
- Environment: Linux 6.8.0-147-generic; Node 26.5.0; pnpm 12.4.1; Svelte 5.56.8; Vite 8.2.0; Playwright 1.64.1; bundled headless Chrome for Testing 156.0.8078.4. Existing Vitest 4.1.10/browser 4.1.12 warning and SQL/WASM chunk/plugin timing notices remain.

Initial new-browser attempts exposed harness waits/labels/state assumptions and a missing nullable fixture default, not product defects. They were corrected and the final suite passes. Quota, denial, lost-write and uncertain-result cases are controlled injections around real persistence, not claims of actual disk exhaustion, power loss or browser policy failure. Exact fault distinctions are in the stable document.

No remaining core acceptance gap at the approved seams. Other engines/mobile/private modes/non-root deployments, physical faults, a historical app binary, selected live reference acceptance, independent security review, download durability and secure erasure remain explicitly unqualified. No Rust/storage source changed, so no Rust/full-monorepo/CRAP command ran. No real data, source profile, build output, screenshot, attachment or dependency link is committed. No root merge or push.

## Comments

- Final handoff: `git diff --check` and integrity checks passed. Parent spec, shared map, runtime/storage source, migrations and lockfiles match `769850d61`; the only package-manifest change is the expanded `test:offline` command. All 37 relative links in changed Markdown files resolve. Repeated both built-browser entry points after final formatting: passed. Merged integration into this task with `GIT_EDITOR=true git merge --no-edit implement/password-manager-parity`: **Already up to date**, integration `769850d61`. No root checkout merge or push.

- 2026-11-05: Claimed in the existing `.scratch/worktrees/password-manager-20`, branch `implement/password-manager-20`, base `769850d61`. Read the full spec, all 19 completed Answers, map/Fog, app/repo rules and related stable contracts. Loaded TDD and both Svelte skills. Use the spec-approved visible application, public real-persistence API, and built offline browser seams. Extend existing harnesses; no shared map or parent edits. Complexity: medium; risk: high (recovery completeness, silent portability loss, false success). All browser/build/check work uses the shared absolute flock; Vitest uses one worker and no file parallelism. No Rust or storage-library source changes planned.
