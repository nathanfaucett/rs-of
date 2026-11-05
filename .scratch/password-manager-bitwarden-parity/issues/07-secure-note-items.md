# 07: Store Secure Note items

**What to build:** Complete encrypted Secure Note creation, reading, editing, cancellation, and reopen without a required login password.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Add Secure Note selection and type-specific validation through the item model, persistence, list, detail, and editor. Start with private plain text; rich text and Markdown preview are not required.
- [x] Preserve applicable notes, favorites, custom fields, identity, and timestamps using the verified field mapping. Do not synthesize a password to satisfy the former Login model.
- [x] Create, view, edit, cancel, save, and reopen a note with Unicode and multiline content. Editing or cancelling one item must not change another.
- [x] Retain encrypted-at-rest storage, complete-snapshot validation, acknowledged writes, protected detail actions, keyboard access, and lock cleanup.
- [x] Extend encrypted backup/clean-storage restore coverage for notes and mixed Login/Note vaults. Test visible workflows and real-persistence reopen with failure cases.
- [x] Preserve offline operation and include any required assets in offline delivery.

## Comments

- Claimed on `implement/password-manager-07` in `.scratch/worktrees/password-manager-07`, base `b6ec0fdce`. Scope: Note end-to-end and explicit-kind union draft/workflow/editor support. Shared editor/workflow/detail changes stay narrow; no map/spec changes.
- Crash recovery: confirmed branch/base ancestry and existing claim; preserved and completed the uncommitted source/tests. The base was also the integration tip on recovery. All browser tests, checks, builds, and Svelte tools used `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`. Vitest used one worker and no file parallelism.

## Answer

Secure Notes now support creation, masked read-only details, explicit reveal/copy, isolated edit/cancel, acknowledged save, lock, and offline reopen. The body is common `notes`, subtype is `0`, and no Login password or credential controls are added. Existing Login behavior and the collection/vault ownership boundaries remain unchanged. No new dependency, storage schema, or asset is required.

### Evidence

- `src/routes/note.test.ts`: real page, SQL/WASM, IndexedDB, and crypto. Covers Unicode/multiline/plain-text content; blank-name rejection; unchanged absent/null/empty bodies; metadata, favorite, custom fields, provenance, history, and creation time; unchanged neighboring Login; lock/reopen; exact mixed-vault clean-storage encrypted backup restore; wrong-password rejection; save/readback failures; and lock during draft/copy/read/write/acknowledgement with late results rejected.
- Final focused command from `apps/password-manager`: `pnpm exec vitest run src/routes/note.test.ts src/routes/detail.test.ts src/routes/page.test.ts src/lib/secrets/editor.test.ts src/lib/secrets/secret-draft.test.ts src/lib/secrets/local-collection.delete.test.ts --maxWorkers=1 --no-file-parallelism` — **47 tests passed, 6 files**. Run under the shared lock. The first focused recovery run also passed 26 tests.
- `pnpm run check` — **0 errors, 0 warnings**. `pnpm run build` — passed. `node scripts/check-offline.mjs` — passed all checks, including keyboard Note create/reveal/cancel/edit, cold offline reopen, Login regression, server-stopped startup, mixed Login/Note clean-profile backup restore, and offline asset fault cases. Run serially under the shared lock.
- Svelte autofixer: no issues or suggestions for `LocalSecretEditor.svelte`, `SecretDetail.svelte`, and `local-workflow.svelte.ts`. `git diff --check` passed.

### Interface for tickets 08–10

- `EditableSecret = Login | SecureNote`; `EditableKind` is its discriminator. `SecretDraft = LoginDraft | SecureNoteDraft`; common controls remain shared.
- `createSecretDraft(record, kind?)` defaults new records to Login; `workflow.edit(null, "secureNote")` starts a Note. Existing records determine their own kind.
- `constructSecret(previous, draft, id, now)` selects the existing strict schema directly. It preserves unchanged optional values and metadata; only Login password changes append history. Workflow save refuses a different draft kind.
- Extend these unions and direct kind branches for later types. Do not add a registry, parallel store, compatibility writer, or new persistence route. Details keep their keyed protected-value lifecycle.
- Current behavior and interfaces are recorded in `docs/password-manager-items.md`.

### Limits and known diagnostics

- The full app Vitest command timed out at the 200-second command limit, with 46 tests passed and no assertion failure reported. That run is not claimed as a full-suite pass. The final focused run above completed.
- Existing Login detail tests emit `derived_inert`; deliberate persistence faults emit collection error logs. The focused tests still pass. The build reports the existing SQL WASM static/dynamic import warning and plugin timing advice.
- No Rust suite or `just crap`: that recipe invokes workspace Rust coverage, outside this task's permitted validation. No real credentials or live browser sessions were used. Other browser engines and independent security review are not established.
- Generated screenshots, attachments, root `.svelte-kit`, and dependency links remain outside the commit. The shared map and parent spec are unchanged.
