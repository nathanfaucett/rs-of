# 05: Transition existing records to typed Login items

**What to build:** A versioned typed-item path, exercised end to end by Login items with optional credentials, without losing existing login data.

**Blocked by:** 01 — Verify the reference feature baseline; 04 — Export and restore complete encrypted backups.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Replace the generic required-password model with a discriminated item model. Deliver Login create/edit/cancel/save/reopen behavior through the real collection and vault.
- [x] Allow username-only or passwordless Login items. Apply verified field validation; retain multiple URIs and stored match settings without claiming autofill.
- [x] Transition existing records without losing identity, usernames, passwords, TOTP keys, URIs, match settings, notes, favorites, custom fields, timestamps, or password history.
- [x] Use backup and a versioned atomic or recoverable transition. On malformed data, interrupted writes, or failed transition, leave the source recoverable and do not silently load a partial vault.
- [x] Retain complete-snapshot validation, single-write acknowledgement, cancel isolation, and authoritative reload for uncertain writes. Delete replaced schema/workflow paths; do not retain parallel legacy writers.
- [x] Test visible existing/new Login workflows, nested edit cancellation, transition failures, real-persistence reopen, and encrypted backup/restore of the new format.
- [x] Check lock cleanup and offline asset requirements for this change. Use existing UI, validation, and storage patterns rather than a new abstraction layer.

## Answer

Implemented the discriminated typed-item foundation and the complete Login workflow. The field/interface contract is in [typed items](../../../docs/password-manager-items.md); the versioned recovery contract is in [encrypted backups](../../../docs/password-manager-backup.md).

- `SecretSchema` is a strict `kind` variant. Exported types/schemas cover `login`, `card`, `identity`, `secureNote`, and `sshKey`, with the exact supported reference field sets. Only Login has an editor. Other kinds are not converted to Login by the workflow.
- Login supports absent/null/empty credentials, username-only and passwordless records, ordered URI rules (including inherited null), all existing supporting fields, nested cancel isolation, and dated prior-password capture. Unchanged optional scalar states survive edits. The collection retains full-snapshot validation, one-mutation acknowledgement, and authoritative reconciliation.
- Migration 3 appends item-format/recovery metadata. Migrations 1/2, derivation, proof bindings, vault IDs, and record IDs remain unchanged. A read-only legacy converter preserves the full logical record, ordered nested values, logical dates, outer record dates, history, and deletion state. Unknown/malformed later records block the whole set before staging; no legacy writer remains.
- The lost-write test exposed a defect in an initial in-place design. It was deleted. Conversion now reuses restore's transactional staging, closes/reopens the destination, and verifies the exact durable snapshot. Only then does a compare-and-publish registry transaction change the pointer, followed by readback. The original source database is retained unchanged. Lost/quota writes or failed reopen cannot damage it; uncertain publication can expose only the original or a complete verified destination. Web Locks serialize preparation/writes; stale handles resolve the published storage before writing.
- Version 2 backups authenticate all typed data, item format, and the retained authenticated version 1 pre-transition backup. Both versions restore in clean storage. The page offers current and pre-transition downloads. Recovery-file validation is independent of current records, so damaged current ciphertext does not block a valid recovery download. Damaged recovery metadata is rejected rather than downloaded as a trusted backup.
- `folderId` and encrypted `importSource: { format: "personal-json-v1", id }` provide the documented membership/source-ID slots. No folder table/UI, importer, duplicate hash index, batch mutation abstraction, or other-type editor was added.
- Lock/unmount disposes workflow/draft state; `close()` retires the handle and clears its key reference. Closed handles cannot decrypt, mutate, export, or repopulate a decrypted result. No JavaScript memory-zeroization or automatic timeout-lock claim is made. No runtime dependency, network fetch, or extra offline asset was added.

### Evidence and validation

- TDD red: the mounted username-only test failed at the required password control. The first real transition test failed at the absent format column. The adversarial lost-write test rejected the in-place implementation; the staged replacement passes. The recovery-metadata test initially returned a damaged file; independent authenticated validation fixes it.
- Final `pnpm test --no-file-parallelism`: **12 files / 78 tests passed**, Chromium with real SQL/WASM, IndexedDB, and Web Crypto. No test timeouts were relaxed. An earlier parallel full run hit two existing create-poll timeouts under concurrent crypto load; final sequential runs pass.
- `src/lib/vault/item-transition.test.ts`: 15 tests cover complete original/typed snapshots, exact source retention, all five field sets, optional values and provenance, clean-storage recovery through both backup versions, unknown nested fields/rules/types, mismatched IDs, malformed history, quota/lost/unconfirmed writes, failed staging reopen, rejected/unconfirmed publication, retry, concurrent handles, stale-handle retirement, damaged-current recovery, and damaged recovery rejection.
- Its mounted real-page test covers failed conversion without partial display or false success, original/current and pre-transition recovery downloads, retry/refresh, nested URI/custom-field cancel, clearing a password/history preservation, username-only and no-credential creation, lock, and reopen through the real collection/workflow. Existing collection tests still verify unacknowledged writes and authoritative reload.
- `pnpm run check`: passed, zero errors/warnings. `pnpm run build`: passed; existing WASM import/plugin-timing warnings remain.
- Final `node scripts/check-offline.mjs`: all gates passed. Built typed Login persistence and version 2 backup/clean-profile restore work offline; cache/update/WASM/storage-error gates pass. Built-app legacy conversion was not separately added; its real-persistence and mounted application tests pass.
- Focused Biome formatting/lint: passed. Svelte autofixer on `LocalSecretEditor.svelte`, `local-workflow.svelte.ts`, and `+page.svelte`, installed Svelte `5.56.8`: no issues/suggestions. Absolute paths were required for module-file analysis.
- `just crap`: attempted; blocked in `sdk-build` by `ERR_PNPM_UNSAFE_TASK_RUN_STATE_PATH` because the required reused `node_modules` is a symlink. No Rust/storage-library implementation changed; Rust feature-powerset tests were not run under the spec's application validation boundary.
- Generated ignored Paraglide files for worktree setup. Root/app dependencies are symlinked to the root checkout, as in prior setups. No install, package/lockfile change, shared map/spec edit, live browser tab, real credential, or push.

### Interfaces and limits for ticket 06 and type tickets

Use the exported type/schema for each kind; narrow union records before accessing type fields. The current draft/workflow `edit` path is Login-only. Reuse the union collection and `await vault.prepareItems()` before direct typed operations. `exportBackup()` is current-format recovery; `exportTransitionBackup()` is the independently verified old source snapshot. Detailed mappings and provenance/import obligations are in the stable items document.

Keep the source and external backup until recovery is verified. Source/staging databases and the embedded recovery snapshot consume storage and retain old/deleted values; no automatic purge/cleanup was added. The complete 16 MiB bound includes embedded recovery bytes. Independent security review, real disk exhaustion, other browser engines, automatic lock, and full import/folder qualification remain separate work.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-05` at integration `572e70671`. Approved seams from the spec: mounted application workflows and public vault API with real persistence. Complexity: medium/high; risk: high (data loss and false write success); priority: preserve the complete source and verify durable transition before exposing typed records. No shared map/spec edits or live browser tabs.
