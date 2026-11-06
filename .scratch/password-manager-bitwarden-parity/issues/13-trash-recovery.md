# 13: Recover deleted items through Trash

**What to build:** Reversible deletion through a Trash view, with restore and separately confirmed permanent deletion.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** resolved

- [x] Replace normal hard deletion with an encrypted persisted deletion state. Normal list/search/filter views exclude trashed items; provide a distinct accessible Trash view.
- [x] Restore retains item identity, contents, history, and metadata. Define a safe unfiled fallback if an item's original folder no longer exists.
- [x] Permanent deletion requires explicit separate confirmation and affects only the selected trashed item. Cancellation changes nothing.
- [x] Do not add automatic purging based on an assumed retention period. Document that policy separately if automatic retention is later requested.
- [x] Test delete, cancellation, Trash navigation, restore, permanent deletion, failed writes, and close/reopen through visible flows and real persistence. Extend coverage for each available item type.
- [x] Include Trash state in encrypted backup/clean-storage restore coverage. Preserve authoritative-write verification and uncertain-outcome feedback.
- [x] Keep deleted content encrypted, clear decrypted Trash state on lock, and preserve offline operation. Remove the obsolete normal hard-delete UI path.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-13` at base `b6ec0fdce`. Use the spec-approved visible application and public vault API with real persistence test boundaries. Complexity: medium; risk: high (data loss, false write success, decrypted state after lock). Scope: existing `deletedAt`, Trash, restore, explicit permanent delete; no automatic purge. Keep diffs narrow for parallel 07/11/12; folder fallback requires final integration with 12. No shared map/spec edits, push, or live tabs.
- 2026-11-05: Recovered the existing claimed worktree after the crash; kept its uncommitted implementation and tests. Confirmed `b6ec0fdce` is an ancestor and the integration tip at recovery. Loaded TDD and both Svelte skills. Added a real-persistence unresolved-folder regression: observed failure retaining the dangling UUID, then passed after clearing only unresolved membership to null in this no-catalog baseline. No owner 11/12 files were changed.

## Answer

Accepted on the ticket 13 baseline. Normal Delete now persists encrypted `deletedAt`; normal records/details exclude Trash. The separate Trash view provides Restore and a separately confirmed permanent delete. Cancel/Escape leaves storage unchanged and returns focus; confirmation initially focuses Cancel. Restore retains all other item data; unresolved folder membership becomes unfiled. No retention timer or automatic purge exists. Existing authoritative-write verification and uncertain-outcome feedback remain intact. Lock clears Trash/dialog state and rejects late feedback.

### Evidence

- `apps/password-manager/src/routes/trash.test.ts`: all five item types; real Chromium SQL/WASM, IndexedDB and Web Crypto; visible cancellation/navigation, encrypted delete, lock/reopen, complete encrypted backup restored after source storage deletion, exact restore, selected permanent deletion, and unresolved-folder fallback. Also write/quota failures, uncertain readback, old deletion dates, and lock during pending trash/restore/purge.
- `apps/password-manager/src/lib/secrets/editor.test.ts`: real workflow/collection with controlled storage; selected mutations, cancel/focus, and unacknowledged writes never report success.
- Final validation passed under the shared `flock` lock: `pnpm exec vitest run --maxWorkers=1 --no-file-parallelism`; `pnpm run check` (0 errors/warnings); `pnpm exec vite build`; `node scripts/check-offline.mjs`. Built offline Trash actions made zero server requests; keyboard focus/Escape, cold reload, history restoration, selected permanent deletion, and clean-profile encrypted backup checks passed.
- Svelte component autofixer returned no issues or suggestions. A later repeat and the optional workflow autofixer timed out; no clean workflow-autofixer result is claimed. An initial build attempt timed out waiting for the lock; the complete locked retry above passed.
- `git diff --check` passed. No full Rust or `just crap`: that recipe builds SDKs and runs workspace Rust coverage; this ticket changes only the browser application and docs, and the user prohibited full Rust.

### Merger needs

- Owner 11: keep `!deletedAt` filtering before normal search/type/favorite/folder/sort processing, exclude deleted details, and keep Trash separate from those filters. Test combined queries before/after trash, restore, refresh, and reopen.
- Owner 12: this baseline has no folder catalog. Replace the explicit no-catalog fallback in `changeTrash` with its authoritative catalog lookup: retain an existing folder ID, otherwise set null. Its atomic folder delete must unfile active and trashed members. Adapt the orphan fixture if complete-snapshot validation rejects direct orphan writes; test folder deletion while trashed, restoration into a retained folder, and folders+Trash backup/clean restore. Do not copy the no-catalog fallback unchanged into a folder-aware build.
- Cross-ticket search/folder combinations require joint acceptance after those owners land; they are not claimed as tested here. See `docs/password-manager-items.md#trash-and-recovery`.
- Commit only ticket 13 application/test/docs files. Screenshots, `.vitest-attachments`, build artifacts, and dependency symlinks are not task changes. No map/spec edits or push.
