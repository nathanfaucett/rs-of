# 16: Enforce manual and automatic lock

**What to build:** Reliable manual/inactivity lock with persisted settings, retired vault handles, and no late return of decrypted state.

**Blocked by:** 02 — Authenticate empty and populated vaults; 03 — Separate vault identity from its name; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** resolved

- [x] Add accessible per-vault lock settings and persistent timeout behavior. Protect sensitive settings; document the supported timeout/restart semantics rather than assuming reference options.
- [x] Use an inactivity deadline checked on activity and tab resume. A suspended timer must not leave an expired vault usable; do not claim ordinary browser code detects operating-system lock reliably.
- [x] Manual and automatic lock retire the active workflow/collection and deny operations through the old vault handle. Release application references to keys, records, drafts, history, generated values, and displayed TOTP codes wherever those features exist.
- [x] Define pending-write behavior. Late read/write/copy callbacks must not restore an unlocked view or report an unsaved change as successful. Preserve authoritative reconciliation where a write may have committed.
- [x] Reopening requires correct authentication and reloads the saved settings. Lock must not erase stored items or prevent encrypted backup recovery.
- [x] Test visible manual/automatic flows, persisted settings, tab suspension/resume, stale API handles with real persistence, active drafts, pending writes, and late callbacks. Extend checks as other features land.
- [x] Keep lock local/offline. State JavaScript memory and clipboard limits honestly; do not add biometric, PIN, or client-side TOTP unlock claims.

## Answer

Completed the full ticket acceptance at the approved mounted-application and real-persistence boundaries.

- Added keyboard-accessible, labeled per-vault settings for 1/5/15/30 minutes, default 5. Unlock starts a deadline; reload/restart requires the master password. Saved settings use purpose- and vault-bound AES-GCM, exact persisted readback, and encrypted backup/restore. Damaged settings are not silently reset.
- Activity checks expiry before extending the deadline or invoking the target action. A one-second timer plus focus/pageshow/visibility checks enforce expiry on resume. A red/green regression prevents an expired pending backup callback from starting a download while the timer is suspended.
- Manual/automatic lock retire the existing workflow/collection, clear drafts/selection/reveals/history references, and close the handle. Added post-await guards for stale read/write/preparation/settings/backup results and idempotent close. Existing session/component disposal guards suppress late feedback. Generator output and displayed TOTP codes do not exist at this base; later features must keep them in the retired subtree.
- Already-sent writes can commit. Lock does not claim cancellation or success. Reopen reloads authoritative records/settings. Tests cover lost acknowledgements, rejected-after-commit settings, committed item/settings/delete results after close, active drafts, and late read/copy/write/backup callbacks.
- Clean-storage backup recovery preserves settings and an item and rejects a wrong password. Backup validation, transactional staging, exact reopen verification, and item-format transition retain settings. Existing version 1/2 backups without settings use the default. Older strict readers reject the extended payload; no old-reader compatibility claim is made.
- Documented timeout/restart semantics, per-tab scope, local/offline behavior, JavaScript memory and clipboard limits, and the lack of reliable OS-lock detection. No biometric, PIN, or TOTP unlock was added. See `docs/password-manager-lock.md` and the linked app/backup/item documentation.

### Validation evidence

All commands ran from this task worktree's `apps/password-manager`, under `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`. All Vitest runs used `--maxWorkers=1 --no-file-parallelism`.

- Focused lock/detail suites: **31 passed**. The new suspended-timer backup regression failed before the deadline guard and passed after it.
- Full app `pnpm test --maxWorkers=1 --no-file-parallelism`: **116 tests passed, 15 files**.
- `pnpm run check`: **0 errors, 0 warnings**.
- `pnpm run build`: **passed**. Linked SQL/WASM static/dynamic import and plugin timing advisories remain non-fatal.
- `node scripts/check-offline.mjs`: **passed**. Built offline create/edit/manual lock/reload/cold-process reopen, stopped-server flows, clean-profile encrypted recovery, wrong password, cache/update/failure checks. Deadline suspension/resume is tested in mounted Chromium flows, not claimed as actual OS-lock detection.
- Svelte autofixer on `LockSettings.svelte` and `+page.svelte`: **no issues or suggestions**.
- `git diff --check`: **passed**.
- One initial regression command timed out waiting for the shared validation lock with no output; the later serialized run completed. No Rust or monorepo suite, real vault, or live browser session was used.

### Merger notes

- Base/ancestry: task started at integration `b6ec0fdce`. Merge integration **into this task**, never into the root checkout; record the final merge result below.
- Both this task and parallel folder 12 append SQL migration **4**. In the combined unreleased migration list, retain one as 4 and append the other as 5. Applied migration histories cannot be rewritten; use only matching development storage until integration. No folder source was edited here.
- Public additions: `VaultHandle.loadLockSettings()` and `saveLockSettings({ timeoutMinutes })`. Lock-settings ciphertext uses AAD `password-manager/lock-settings/v1/<vault ID>`.
- Backup adds optional nullable `lockSettings` inside authenticated version 1/2 payloads; item format stays 2. Folder 12 adds envelope version 3 and `folders`. Preserve both data sets through snapshot, validation, staging, exact readback, transition, and **every folder-mutation staging payload**. Apply closed-handle guards to folder APIs and rerun combined lock/folder/backup tests after merge. Details are in `docs/password-manager-lock.md`.
- Shared map/spec and generated build/test artifacts are excluded from the task commit.

## Comments

- 2026-11-05: Final integration merge executed from this task worktree: `git merge --no-edit implement/password-manager-parity` → **Already up to date** (integration `b6ec0fdce` is an ancestor). Task code commit: `feat(password-manager): enforce encrypted inactivity lock settings`. No root checkout merge, push, map/spec edit, or artifact commit.

- 2026-11-05: Recovered after crash. Preserved the saved task edits and inspected their diff. Confirmed `b6ec0fdce` is an ancestor of task HEAD and integration `implement/password-manager-parity` is at that base. Retained the approved mounted-app/public-persistence test boundaries. Loaded TDD and Svelte skills. All browser/test/check/build work uses the shared validation lock, one Vitest worker, and no file parallelism. Folder 12 has a competing migration 4 and backup version 3; merger instructions are in `docs/password-manager-lock.md`. No folder source, shared map/spec, root checkout, or live browser is changed.

- 2026-11-05: Claimed on `implement/password-manager-16`, base `b6ec0fdce`, worktree `.scratch/worktrees/password-manager-16`. Scope: lock/settings module and component, page integration, vault close guards. Approved test seams: mounted application flows and public vault API with real persistence, as specified in the full spec. No map/spec edits; no live tabs. Complexity: medium; risk: high (late callbacks, uncertain writes, encrypted settings/backup); priority: prevent stale access and false success.
