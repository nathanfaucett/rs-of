# 11: Search, filter, and sort unlocked items

**What to build:** Combined text search, type/favorite filtering, and clear sorting of the unlocked vault without a persistent plaintext index.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** resolved

- [x] Search item names, usernames, websites, notes, and custom-field names from the unlocked in-memory view. Do not introduce persistent plaintext search caches or FTS tables.
- [x] Combine text search with type and favorite filters and explicit sort controls. Define stable ordering and accessible empty/loading/error states.
- [x] Keep results current after acknowledged create/edit/delete/refresh operations. Navigation to a result must use the existing detail flow without silently editing it.
- [x] Search only available item types; additional type tickets extend coverage as they land. Folder filtering is delivered with folders; Trash integration must exclude deleted items from normal results.
- [x] Test visible combined queries, clearing filters, Unicode, missing optional data, ordering, updates, and empty results using the real workflow/collection. Use representative synthetic large vaults without an invented timing guarantee.
- [x] Clear decrypted query results on lock. Verify no plaintext index persists and all operations remain local/offline.

## Comments

- 2026-11-05: Claimed in `implement/password-manager-11`, base `b6ec0fdce`. Scope: dedicated memory-only search module and editor list controls/tests. Use the approved mounted application seam with the real workflow/collection. Leave draft/detail, folder integration (12), and Trash actions (13) to their owners. Complexity: low; risk: hidden results or retained query state; priority: local navigation without storage changes.

## Answer

Implemented memory-only search, combined type/favorite filters, and explicit sorting on `implement/password-manager-11`.

### Behavior and ownership

- `apps/password-manager/src/lib/secrets/search.ts` owns the pure search projection. It reads the current validated `Secret` union, allocates only transient field lists/results, and neither mutates nor stores source records. There is no persistent index, SQL migration, cache, network call, new package, or backup change.
- Search matches trimmed literal substrings after NFC normalization and locale-independent lowercase. Allowed fields: item name, notes, custom-field names, Login/Identity usernames, and Login website text. Tests reject matches on passwords, TOTP, history, field values, card numbers, identity identifiers, and SSH key material. Missing optional values are safe.
- All five defined kinds work now through real workflow/collection tests, including those without creation UI. `deletedAt` records are excluded from normal results. Folder integration belongs to merger 12; Trash actions belong to 13.
- Default sort: favorite descending, normalized name ascending. Other choices: name ascending/descending, update instant descending, creation instant descending. Date ties use normalized name ascending; all final ties use ID ascending. Names use deterministic JavaScript string order rather than locale dictionary collation.
- `LocalSecretEditor.svelte` changes are limited to the search import/state/retirement and list controls/results. Labelled native controls, a polite result-count live region, status empty states, and existing loading/read-error states are keyboard accessible. Clear filters resets text/type/favorite and retains sort. Results use the existing read-only detail flow and dirty-navigation guard. Draft, detail, workflow, and storage implementations are unchanged.
- Create/edit/delete/refresh results follow the real collection. Read failure hides stale results, disables search controls, and permits retry with the same filters. Retirement/unmount clears query/filter/result references and disposes the workflow; late refresh cannot repopulate the view. No guaranteed JavaScript memory erasure is claimed.
- `docs/password-manager-items.md` records the contract and qualification limits. Shared map/spec were not changed.

### Test-first evidence

The first mounted test failed with `Missing control: Search items` before implementation. It then passed with combined query/type/favorite filtering, clearing, empty results, read-only selection, and no vault writes. Test setup uses the existing mounted editor pattern, real workflow/collection, and a synthetic vault adapter. Further mounted cases cover all union kinds, Unicode, omitted optionals, allowed/excluded fields, deleted records, every sort, ID ties, acknowledged create/edit/delete/refresh, read failure/retry, retired pending refresh, and 2,000 synthetic records. No invented latency guarantee is used.

The built-app check adds `scripts/check-search.mjs` through a small `check-offline.mjs` hook. It uses a new synthetic Chromium profile with networking disabled and the server stopped, real SQL/WASM persistence, keyboard search/filter/sort/clear, read-only result navigation, and unchanged local/session storage, database registrations, and URL. Lock removes the search controls/results. Existing cold-start/reopen and clean-profile encrypted backup restoration still pass. These storage assertions are not a general audit of browser storage contents.

### Exact checks

Commands below ran from `apps/password-manager` unless stated otherwise.

Passed:

- `pnpm exec vitest run src/lib/secrets/search.test.ts src/lib/secrets/editor.test.ts` — 2 files, 17 tests passed.
- Final repeat: `pnpm exec vitest run src/lib/secrets/search.test.ts src/lib/secrets/editor.test.ts --no-file-parallelism` — 2 files, 17 tests passed.
- `pnpm run build` — static production build passed.
- `pnpm run check` — 0 errors, 0 warnings (also repeated after documentation work).
- `node scripts/check-offline.mjs` — added search check plus all existing offline, lock/cold-start, cache/update/fault, and encrypted backup/clean-profile restore checks passed.
- `pnpm exec biome check src/lib/secrets/search.ts src/lib/secrets/search.test.ts scripts/check-search.mjs` — 3 files passed after formatting only these new files.
- `pnpm exec biome lint src/lib/secrets/LocalSecretEditor.svelte src/lib/secrets/search.ts src/lib/secrets/search.test.ts scripts/check-search.mjs scripts/check-offline.mjs` — 5 files passed.
- Svelte autofixer for `LocalSecretEditor.svelte`, version `5.56.4` — no issues or suggestions, including final repeat.
- `git diff --check` at worktree root — passed.

Not passed:

- `pnpm test` — 8 files passed, 6 failed; 84 tests passed, 14 failed. Failures included backup persistence timeouts (`backup.test.ts`, 15/20-second limits) and the existing mounted transition test remaining at `Loading…` (`item-transition.test.ts:395`). No full-suite success is claimed.
- `pnpm exec vitest run --no-file-parallelism` — exceeded the 240,000 ms tool limit; partial summary: 6 files passed, 1 failed, 50 tests passed, 5 failed, before completion. Authentication/backup timeouts and screenshot timeouts occurred. No unrelated tests, timeout limits, or dependency versions were changed.
- Vitest consistently warns that installed `vitest@4.1.10` and `@vitest/browser@4.1.12` are mixed versions. This existing dependency mismatch remains.

Environment setup: `pnpm install --offline --frozen-lockfile` reused cached packages. The worktree-relative `@ofsdk/sql` link was broken; only the ignored `node_modules/@ofsdk/sql` symlink was corrected to the existing local package. `pnpm --filter @ofsdk/svelte-headless run build` built the existing UI dependency. No manifest or lockfile was changed. Initial pre-test startup failures from these missing build/link prerequisites are not counted as test-first red evidence. The initial type check also needed generated Paraglide output, provided by the normal production build.

### Limits and handoff

No live tabs, real vault data, push, root/integration merge, Rust/storage changes, folder implementation, or Trash implementation. Full-suite failures remain a qualification limit; focused and built offline/backup checks are the passing evidence. The merger must preserve the search projection when adding folder/Trash controls and retain the existing detail/draft ownership boundaries. Integration-tip handling and final commit ID are reported in the delivery response.
