# 08: Store Card items

**What to build:** Complete Card item management with relevant payment fields, protected display, and encrypted local persistence.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Use the verified Card field mapping and document validation decisions. Do not invent mandatory fields or reject valid stored/imported data solely from an unverified brand or checksum assumption.
- [x] Deliver type selection, create, list, detail, edit, cancel, and save through the typed item model and real storage. Card items must not require a login password.
- [x] Mask sensitive payment values by default and support explicit reveal/copy with safe feedback and keyboard access. Preserve applicable common metadata and custom fields.
- [x] Test optional fields, representative valid/invalid input, Unicode, nested cancellation, isolated updates, write errors, and close/reopen using synthetic payment data only.
- [x] Extend encrypted backup/clean-storage restore tests for Cards and mixed-type vaults. Verify sensitive values are not stored in plaintext or logged.
- [x] Preserve lock cleanup, authoritative-write verification, and offline operation/assets.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-08`, based on `30c7f7c8c`, in the assigned worktree. Use the spec-approved application, public real-persistence, and built offline test boundaries. Complexity: low UI/model extension; risk: payment disclosure and optional-value loss; priority: preserve protected actions and verified persistence. No new storage format or dependency.


## Answer

Implemented Cards through the existing typed union, isolated draft, workflow, editor, protected details, and verified encrypted persistence. **New card** selects the type. `CardFields.svelte` keeps per-kind UI separate from the parallel Identity and SSH work; common notes/favorite/custom fields remain common. No new registry, dependency, storage format, or compatibility writer.

All six payment fields are optional strings. The common name is required. Retain absent/null/empty values, original formatting, leading zeroes, Unicode, unknown brands, and expired or out-of-range dates. No brand/checksum/date assumptions reject preserved data. Non-string fields and unknown keys fail strict validation before typed writes. Number/code are masked by default with explicit reveal/copy and fixed accessible clipboard feedback. Card edits retain common metadata/history and do not append Login history. Validation decisions and evidence are in `docs/password-manager-items.md`.

Acceptance evidence:

- `src/routes/card.test.ts`: 12 passing mounted real-page tests using the actual workflow, collection, Chromium SQL/WASM, IndexedDB, and Web Crypto. Covers full CRUD except deletion (existing Trash tests retained), optional/minimal/invalid fields, Unicode, nested cancel, isolated Login preservation, common metadata, protected copy/reveal, failure feedback, no sensitive storage/backup/log markers, lock/late-result cleanup, close/reopen, and exact mixed-vault backup recovery after source storage deletion.
- `scripts/check-card.mjs` extends the existing offline and backup checks. Built app passes Card create, keyboard reveal/edit/cancel and nested cancellation, cold offline reopen, and clean-profile mixed Login/Note/Card restore. The offline server is stopped during the main flow; source browser context is closed before clean-profile restore. Exact backup payload comparison retains folders, encrypted settings, and all encrypted item records.
- Red first: the first mounted Card flow failed on missing **Show Card number**, then passed after implementation. New invalid-write test was corrected to prepare the typed API before testing its documented strict write boundary.
- Full app suite: `node_modules/.bin/vitest run --maxWorkers=1 --no-file-parallelism` — **26 files, 257 tests passed**.
- `node_modules/.bin/vite build` — passed; `node_modules/.bin/svelte-check --tsconfig ./tsconfig.json` after generated locale assets — **0 errors, 0 warnings**.
- `node scripts/check-offline.mjs` — passed, including existing generator, search, Trash, cache/update/WASM, stopped-server new-process startup, folders/settings and backup checks.
- Svelte MCP autofixer on `CardFields.svelte`, `LocalSecretEditor.svelte`, and `SecretDetail.svelte` — no issues or suggestions.
- Every test/browser/build/check used the shared absolute `flock` path. An offline retry timed out waiting for another task's validation lock; later serialized validation completed. Initial local dependency/generated-asset setup and browser selector timing were corrected; no package or lockfile changes are part of this task.

Limits: synthetic-only Chromium, not live payment data or deployed-reference parity. Existing mixed Vitest/browser-version and SQL dynamic-import warnings remain. No full Rust/monorepo tests or complexity run: no Rust/storage-library code changed, and the user excluded broad validation. Clipboard retention and JavaScript memory erasure retain their documented limits.

Merger notes: preserve the Card branches in `secret-draft.ts`, `local-workflow.svelte.ts`, `LocalSecretEditor.svelte`, and `SecretDetail.svelte` when combining tickets 09/10. Keep the dedicated field component and existing common fields once. Keep `check-card.mjs` calls in the offline and backup scripts, including Card folder assignment before the unfiled-view assertion. Do not merge this branch into root or push as part of this task. Map/spec unchanged at user request.
