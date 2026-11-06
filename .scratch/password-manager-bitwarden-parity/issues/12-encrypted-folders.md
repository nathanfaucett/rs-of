# 12: Organize items in encrypted folders

**What to build:** Vault-local encrypted folders with create/rename/delete, item movement, and a folder-filtered view that never deletes contained items by mistake.

**Blocked by:** 01 — Verify the reference feature baseline; 06 — Add item details and safe credential actions.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Persist folders and item assignments encrypted at rest. Provide accessible folder navigation, create, rename, move, unfile, and filter actions through the application.
- [x] Use verified naming/nesting semantics. Do not claim hierarchical parity before evidence supports it; do not add organizations or shared collections.
- [x] Confirm folder deletion and leave contained items accessible as unfiled items. Failed move/delete operations must not produce orphaned or lost records.
- [x] If multiple records must change together, use an atomic or explicitly recoverable persistence operation rather than treating a loop of single-item writes as atomic.
- [x] Folder filtering works independently and composes with search/type/favorite controls when those controls are available, regardless of ticket landing order. Cover empty and unfiled views.
- [x] Test user-visible workflows, cancellation, name collisions, failed writes, lock/reopen, and vault isolation. Extend backup/clean-storage restore tests for folders and assignments.
- [x] Preserve complete-snapshot validation, authoritative-write verification, lock cleanup, and offline operation.

## Answer

Recovered and completed the existing task work. The [folder operation, API, recovery, backup, integration, and evidence contract](../../../docs/password-manager-folders.md) records the implementation and limits.

- Encrypted folder records and encrypted item membership use one complete validated snapshot. Duplicate names retain separate UUIDs; Unicode, whitespace, and slash names are literal. No hierarchy, organization, or shared collection is claimed.
- Create/rename, exact/all/unfiled filtering, move/unfile, explicit folder-delete confirmation, cancellation, and empty views work through the real page. Deletion clears direct membership, including Trash, without deleting items or child-name folders. Folder/item drafts share the existing navigation guard; only one edit form can remain active. Cancel restores focus; lock retires folder drafts and state.
- `readItems()` supplies the collection's item/folder snapshot. `mutateItems()` is the importer's one-batch boundary for item/folder upserts and removals. Folder helpers use the same boundary. Complete SQL staging, close/reopen exact verification, compare-and-publish registry transaction, registry readback, stale-handle resolution, and Web Locks prevent publication of a partial set. Uncertain outcomes report failure and reload authoritative data; no source is deleted.
- Backup version **3**, with typed `itemFormat: 2`, authenticates encrypted folders and all assignments, including empty folders. Versions 1 and 2 retain their original meanings; the embedded pre-transition file remains version 1. Clean-profile tests compare the complete encrypted payload after restore. SQL migration **4** adds folders; migrations 1–3 are unchanged.
- Folder membership filters `workflow.records` before caller search/type/favorite/sort transforms. Ticket 11's existing branch uses that getter as `searchItems` input. Its controls are not yet on this integration base; preserve the composed input and run combined UI tests when merging that ticket. No parallel item store or plaintext index was added.

### Validation evidence

All browser/test/build/check commands ran serially under `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`.

- `node node_modules/vitest/vitest.mjs run --maxWorkers=1 --no-file-parallelism`: **15 files, 100 tests passed** after final source changes. Includes eight public folder/persistence cases and the mounted real folder workflow, plus all existing app suites.
- `node node_modules/@sveltejs/kit/svelte-kit.js sync` and `node node_modules/svelte-check/bin/svelte-check --tsconfig ./tsconfig.json`: **0 errors, 0 warnings**.
- Svelte autofixer: no issues or suggestions for `FolderControls.svelte`, `LocalSecretEditor.svelte`, and `local-workflow.svelte.ts`.
- `node node_modules/vite/bin/vite.js build`: passed. Only the existing linked SQL/WASM chunking and plugin timing notices remain.
- `node scripts/check-offline.mjs`: passed after the final backup-test change. Covers built offline folder keyboard creation/cancel focus, move/filter/rename, empty folders, clean-profile backup recovery, exact encrypted folder/item payloads, and the retained stopped-server/cold-process/update/cache/fault checks.
- Focus/navigation regression tests were observed failing before the fixes. Earlier full-suite failures from form assumptions, fixed migration-version fixtures, and the changed acknowledgement point were corrected without dropping their original persistence/lock assertions. The keyboard backup test initially focused a still-disabled control; it now waits for the visible save acknowledgement and passes.
- One validation attempt timed out while waiting for the shared lock; it produced no test result and was rerun. Vitest reports installed version skew (`4.1.10` / browser `4.1.12`); dependency files were not changed. The complete suite passes despite that existing warning.
- No Rust/shared-storage implementation changed. Rust/monorepo builds and `just crap` were not run, to avoid the prohibited monorepo load.

### Integration handoff

`GIT_EDITOR=true git merge --no-edit implement/password-manager-parity` ran in this task worktree and returned **Already up to date** at `b6ec0fdce63af502bc98b30bf753954073956b77`. No root merge or push was performed. No shared map/spec edits, generated artifacts, or dependency symlinks belong in the task commit.

For the importer: call `readItems()` for the authoritative destination, remap to fresh UUIDs, then call `mutateItems({ upsert: items, folders })` once after complete preview/validation. `folders` is an upsert list, not replacement. Invalid membership or ambiguous IDs rejects the complete batch. Missing items/folders in move helpers are errors. The primitive batch API does not implement external duplicate policy or preview.

For the merger: ticket 16 has parallel settings/migration work. This task deliberately keeps migration 4 and backup version 3. Resolve version collisions in integration; preserve folders, settings, exact staging verification, and backup validation rather than dropping either payload. Preserve `previousDatabaseName` across later batch publications. Combine ticket 11's search results with the folder-filtered `workflow.records` and retain its result-count/empty-state controls.

Risk: complete-snapshot staging retains encrypted predecessor databases and consumes extra space on each write. No automatic cleanup, secure erasure, independent security audit, actual disk-exhaustion test, or other-browser claim is made. Keep an external backup. Shared map updates are left to the merger as requested.

## Comments

- 2026-11-05: Recovered the existing uncommitted task work after the crash. Confirmed branch `implement/password-manager-12`, worktree `of/.scratch/worktrees/password-manager-12`, and integration/base `b6ec0fdce63af502bc98b30bf753954073956b77`. Claim retained. Full spec, ticket, app/repo rules, reference/JSON, typed-item and backup contracts read. TDD and both Svelte skills loaded. All browser tests/build/checks use the shared validation lock; Vitest uses one worker and no file parallelism. Ticket 16's parallel migration/settings work stays separate; no shared map/spec edits.

- Claimed on `implement/password-manager-12` at `b6ec0fdce`. Scope: existing vault persistence, atomic item/folder mutations, folder UI, complete backup/restore. Test seams: approved real application workflows, public vault API with real persistence, and built offline application. Risk: high (multi-record data integrity and backup completeness); keep the current owner boundaries. Overlap: 13 deletion, 11 list controls, 07 editor union. No map/spec edits.
