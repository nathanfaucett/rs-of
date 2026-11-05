# 09: Store Identity items

**What to build:** Complete Identity item management with verified contact/identity fields and preservation of optional data.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** resolved

- [x] Implement the verified Identity field mapping across validation, encrypted persistence, list, detail, and editor. Avoid locale-specific assumptions that reject supported international names, addresses, or telephone values.
- [x] Support create, view, edit, cancel, save, and reopen without requiring a login password. Preserve optional values and applicable common metadata/custom fields.
- [x] Protect sensitive identity values in detail and copy actions. Provide accessible labels, focus, validation, and failure feedback; do not log personal data.
- [x] Test synthetic international and sparse records, Unicode, multiline values where supported, cancellation, isolated updates, failed writes, and real-persistence reopen.
- [x] Extend encrypted backup/clean-storage restore coverage for Identity items and mixed-type vaults, including optional-field preservation.
- [x] Preserve lock cleanup, complete-snapshot validation, acknowledged writes, and offline operation/assets.

## Answer

Implemented on `implement/password-manager-09` from integration `30c7f7c8c`, in the assigned worktree. TDD and both Svelte skills were loaded. The first application test failed on the missing Identity reveal action before implementation, then passed.

- All eighteen verified Identity fields use the existing Identity schema and encrypted persistence. The editable union, isolated draft, workflow, editor, list/detail selection, and protected actions now support Identity. `IdentityFields.svelte` and a keys/labels-only list keep the shared-file changes small. No plugin, second model, storage layer, migration, runtime dependency, or asset was added.
- Create/view/edit/cancel/save/reopen needs only a name, not a Login password. International names, localized titles/countries, Unicode, leading zeroes, free-form contact values, and multiline address/notes remain intact. Unchanged absent/null/empty values retain their state. Common metadata, fields, existing history, folder membership, and import provenance remain intact; neighboring items do not change. Identifier entry is masked and single-line; existing multiline values survive unrelated saves.
- Identity detail values and notes are masked by default. Existing protected copy/reveal controls provide fixed success/failure feedback and reject late feedback after lock/navigation. Existing list username and common custom-field display rules remain unchanged. Focus, required-name validation, dirty navigation, and failure handling use the current controls/workflow.
- `src/routes/identity-items.test.ts` adds 13 mounted real-page/workflow/SQL/WASM/IndexedDB/Web Crypto tests. They cover every field, sparse records, isolated cancellation/update, clipboard errors, encryption/readback failures, expired Save, manual lock during draft/copy/read/write/acknowledgement, and authoritative reopen. Clean-storage backup recovery compares the full mixed item/folder snapshot, including full and absent/null/empty Identity records, and encrypted lock settings; wrong passwords fail.
- `scripts/check-identity.mjs` extends existing offline and backup checks. Built-app keyboard creation, reveal, cancellation, cold offline reopen and edit pass with the server stopped. Clean-profile offline restore retains all Identity values and the exact authenticated encrypted payload. The asset-cache check excludes the Identity marker. Complete-snapshot validation, acknowledged writes, and all previously merged regression tests remain in place.
- Complexity: one direct kind branch at each existing boundary plus one dedicated form. Risks are optional-value loss, personal-data exposure, and late write results; real-persistence, protected-DOM, failure, and lock tests cover these paths. Priority is the existing core five-type acceptance scope. Operations and evidence are in `docs/password-manager-identity.md`.

### Validation evidence

All browser/test/build/check commands used `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`. Vitest used one worker and no file parallelism. Commands below ran from `apps/password-manager` unless noted.

- `pnpm test --maxWorkers=1 --no-file-parallelism`: **26 files, 258 tests passed**, including the 13 Identity tests and existing authentication, transition, snapshot, folder, Trash, backup, generator, TOTP, and lock coverage.
- `pnpm test src/routes/identity-items.test.ts --maxWorkers=1 --no-file-parallelism`: **13 passed** again after formatting.
- `pnpm run build`: **passed**; static application built with the existing SQL/WASM assets.
- `pnpm run check`: **0 errors, 0 warnings**, including a repeat after formatting.
- `node scripts/check-offline.mjs`: **passed**, including Identity, existing Login/Note/tools/folder/Trash flows, zero-request server-stopped operation, new-browser-process startup, source-profile disposal, clean-profile encrypted restore, update/eviction behavior, and injected storage faults.
- Svelte autofixer on `IdentityFields.svelte`, `LocalSecretEditor.svelte`, and `SecretDetail.svelte`: **no issues or suggestions**.
- Focused Biome formatting and repository `git diff --check`: passed. No generated build/test artifacts are included in the task commit.

Initial validation waits timed out behind other serialized checks; those commands did not execute. Initial worktree setup needed an ignored local SQL-package symlink correction and the existing headless UI package's `prepack`; no dependency manifest changed. An initial check lacked generated locale files; the normal app build generated them and subsequent checks passed.

No full Rust or monorepo validation was run, as requested. No live vault or enabled user browser tab was used. Other browser engines, actual disk exhaustion, JavaScript secure erasure, clipboard removal, and independent security review remain outside this evidence. Map and spec are unchanged.

## Comments
