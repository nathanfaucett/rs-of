# 06: Add item details and safe credential actions

**What to build:** A read-only item detail flow with explicit editing, safe reveal/copy actions, and usable dated password history.

**Blocked by:** 05 — Transition existing records to typed Login items.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Opening a Login item shows details without entering edit mode or changing persistence. Sensitive values start masked; reveal is explicit and resets on navigation and lock.
- [x] Provide explicit copy actions with success/failure feedback. Handle denied or unavailable clipboard access. Any clipboard clearing is best effort and must not overwrite newer content.
- [x] Support explicit edit, save, cancel, and unsaved-edit handling. Cancel must discard nested edits and leave stored data unchanged.
- [x] Show dated password history with protected reveal/copy behavior. Record actual password changes, not notes-only edits or abandoned drafts.
- [x] Use keyboard-operable controls, clear labels, predictable focus, and accessible error feedback. Do not expose credentials in URLs, notifications, logs, or telemetry.
- [x] Test the real workflow/collection through visible actions, including clipboard failure, history preservation, cancellation, navigation, lock cleanup, and late asynchronous results.
- [x] Preserve backup/restore and offline operation. Keep the detail/edit pattern usable by subsequent item types without speculative plugin infrastructure.

## Answer

Implemented all acceptance items. See [typed item interfaces and detail operations](../../../docs/password-manager-items.md#detail-editing-and-protected-actions).

- Item selection is read-only and stores only an ID. Details derive from the real collection. Explicit Edit creates the existing isolated Login draft; Cancel returns to masked details without writes. Save keeps acknowledgement/reconciliation and actual prior-password capture. Notes-only changes and abandoned drafts do not add history.
- One `ProtectedValue.svelte` control serves passwords, TOTP seeds, hidden fields, and dated history. Masked values are absent from DOM values/attributes/text. Explicit reveal/copy uses native buttons, fixed accessible success/failure messages, and handles denied/unavailable clipboard access. Late copy completion is ignored after navigation/unmount. No automatic clipboard clear is attempted: browser APIs cannot atomically compare and clear, and read-then-write could overwrite newer content. Messages state that copied values can remain.
- Dirty item/new/refresh/delete navigation asks before discard. Document/route navigation guards unsaved or pending work. Block-local draft bindings prevent stale input events after cancel/retirement. Attachments place focus on detail headings, edit names, and delete confirmation; closing details/new-draft cancellation returns focus to New secret. Top alignment keeps long detail pages reachable.
- Manual lock is immediate even during collection work or backup. It retires the workflow, clears pending deletion/drafts/selection, closes the handle, and warns that an already-sent write may have persisted. Late copy/read/write/reconciliation results cannot restore secrets or claim success. Tests prove both a prevented pre-write and an already-committed write before acknowledgement. Session checks also reject late backup downloads/errors and close late create/unlock handles after page destruction.
- No new dependency, persistent plaintext state/index, URL state, remote call, storage format, or duplicate item schema/store. No runtime asset was added. Existing transition/backup and authoritative collection paths remain in use.

### Evidence and validation

- TDD red: the read-only test failed because item opening rendered the edit form; the pending-backup test failed because Lock was disabled; the late-unlock test failed because the returned handle was not closed. All pass after root-cause changes.
- The real mounted persistence test exposed a stale input-binding rejection on cancel; block-local editor/draft bindings fix it. The built offline test exposed centered overflow covering list controls; top alignment fixes it without forced clicks.
- Final `pnpm lint && pnpm check && pnpm test --maxWorkers=1 && pnpm test:offline`: passed. Full Chromium suite: 13 files / 91 tests; Svelte check: zero errors/warnings. Lint retains generated Paraglide warnings/info; existing mixed Vitest version and build WASM/plugin-timing warnings remain. No timeouts were relaxed. An initial full parallel run during a concurrent build hit two create-vault polling timeouts; sequential final validation passed.
- `src/lib/secrets/editor.test.ts`: real workflow/collection, no-write details, optional Login, protected reveal and reset, clipboard success/denial/unavailability, hidden-field copy, late navigation completion, nested dirty navigation rejection/approval, unload prevention, focus, save/cancel, history, and deletion regression checks.
- `src/routes/detail.test.ts`: real page, SQL/WASM, IndexedDB, and Web Crypto; exact read-only persistence, nested/password cancellation, notes-only and dated changed-password history, protected history copy, failed save/recovery, lock/reopen, clean-storage exact encrypted backup restore, and gated pending copy/read/write/acknowledgement cleanup.
- `src/routes/page.test.ts`: late create/unlock handle closure and late backup result rejection. Existing transition, authentication, identity, backup, draft, and collection failure tests pass.
- Built offline checks: normal pointer/keyboard detail/edit/reveal/history actions, zero successful server requests, lock/reload and cold browser process with server stopped, encrypted downloads and empty/populated clean-profile restore, cache update/eviction and storage-error gates. Synthetic test browsers only, not a live browser session.
- Svelte autofixer: installed Svelte 5.56.8; `LocalSecretEditor.svelte`, `SecretDetail.svelte`, `ProtectedValue.svelte`, `local-workflow.svelte.ts`, and `+page.svelte` have no issues/suggestions.
- `just crap`: attempted; blocked by the worktree-relative Rust dependency `/of/.scratch/worktrees/ofdb/crates/sql/Cargo.toml` not existing. No Rust/storage-library implementation changed; Rust feature-powerset tests were not run under the spec's application validation boundary. Dependency installation was offline; an ignored local SQL package link was corrected for this worktree. No manifest/lockfile or parent/shared-map edits.

### Interfaces for parallel tickets 07–10

- Keep `SecretSchema`/`Secret` and the existing per-kind schemas in `secret.ts` as the single model. Storage/backup already accept all five kinds.
- `workflow.open(id | null)` selects common read-only detail; `workflow.detail` is `Secret | null`; `workflow.dirty` reports draft changes. Existing `edit`/`SecretDraft`/`constructSecret` remain **Login-only**, intentionally refusing other kinds. Extend them with direct discriminated-kind branches when adding each editor; do not cast other kinds to Login or introduce another model/store.
- `SecretDetail({ record: Secret, canEdit, onedit, onclose })` owns shared notes/custom fields/dates/history once. Add each type's direct kind branch for its fields and Edit availability. Common data needs no duplicate renderer.
- `ProtectedValue({ label, value: string | null | undefined, protected?: boolean })` defaults to masked and supports multiline values. Use it for sensitive Card/Identity/SSH values. Null/absent disables copy; empty string stays copyable. Keep the detail subtree keyed by the selected record to reset state.
- `LocalSecretEditor.hasUnsavedChanges()` includes dirty drafts/pending collection work; `retire()` is synchronous and idempotent. The page uses both for navigation and lock. Preserve these contracts and the session counter when ticket 19 adds automatic lock.
- Other kinds currently expose common details plus an explicit type-specific-unavailable message, not a placeholder Login editor. No plugin infrastructure or speculative type-specific UI was added.

### Limits

No automatic clipboard clearing or secure JavaScript erasure guarantee. No automatic lock, independent security review, real credentials, non-Chromium qualification, push, live browser use, shared map/spec/parent edits, or other-ticket resolution.

## Comments

- Claimed on branch `implement/password-manager-06`, based on integration `d2149f5e4`. Use the spec-approved visible application and public persistence test boundaries. Scope: detail/edit separation, protected actions/history, discarded drafts, navigation guards, lock/late-result cleanup, backup and offline checks. No shared map or parent edits. Complexity: medium; highest risks are plaintext exposure and lost edits.
