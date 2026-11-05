# 04: Export and restore complete encrypted backups

**What to build:** Explicit encrypted backup download and restore into clean local storage without depending on hidden source-browser metadata.

**Blocked by:** 02 — Authenticate empty and populated vaults; 03 — Separate vault identity from its name.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Define a versioned self-contained backup with derivation parameters, salt, authentication material, encrypted records, and authenticated metadata. Never export an unprotected key.
- [x] Provide visible export and restore actions, validation feedback, destination identification, and collision handling. Do not overwrite an existing vault without an explicit safe contract.
- [x] Validate/authenticate the complete backup before committing restore. Wrong passwords, missing metadata, tampering, truncation, unsupported versions, quota failures, and write failures must not leave a visible partial destination or false success.
- [x] Demonstrate restore in clean storage with no source vault available, for both empty and populated vaults. Compare all supported logical data after close/reopen.
- [x] Use synthetic sensitive markers to verify that the backup does not expose plaintext item contents. Never log decrypted data or passwords.
- [x] Test through visible application actions and the public vault API with real persistence. Preserve authoritative-write verification and actionable uncertain-outcome handling.
- [x] Replace the incomplete encrypted-record export rather than wrapping it. Document the format and recovery limits in a stable topic document.

## Answer

Implemented the complete encrypted backup and clean-storage restore in `apps/password-manager/src/lib/vault/backup.ts`, `vault.ts`, `registry.ts`, `RestoreBackup.svelte`, and the page. Removed the incomplete `exportEncrypted()` implementation. Shared crypto/authentication now lives in `crypto.ts`; source derivation, proof bindings, and migrations 1/2 remain unchanged.

- Version 1 includes fixed derivation parameters/salt and a purpose-bound AES-GCM envelope around the original metadata, authentication proof, display name, and complete encrypted record set. No raw key, password, or plaintext item is exported. All records and their inner/outer IDs are validated before export or restore writes. Limits: 16 MiB / 10,000 records.
- Visible download, masked-password validation, authenticated destination ID/name/count preview, confirmation, cancellation, collision guidance, and failure/retry feedback are implemented. Existing IDs, including pending reservations, cannot be overwritten; duplicate labels do not merge identities.
- Restore writes metadata/proof and all records in one SQL transaction to fresh unlisted storage. It closes/reopens and compares the exact authoritative persisted snapshot before one atomic registry publication and registry readback. Rejected/unconfirmed writes never claim success. Uncertain publication errors direct the user to check the selector/ID before retrying.
- Original vault/record IDs, salt, IV, dates, proof, ciphertext, nonces, all supported Login logical fields, and deleted records survive restoration. Existing source data is never deleted or replaced.
- Stable format/API/operations/recovery limits: `docs/password-manager-backup.md`, linked from the app README. Failed staging can leave encrypted unlisted storage; automatic staging cleanup is not claimed. Backup uses the original master password and does not prevent rollback/offline guessing or replace independent security review.

### Evidence and validation

- TDD red: public backup test initially failed because the new API did not exist. Mounted page test failed at the missing **Download encrypted backup** action. Both pass after implementation.
- `pnpm test`: passed, **11 files / 62 tests**, Chromium with real SQL/WASM, IndexedDB, and Web Crypto. Includes 11 backup API and 3 mounted-page tests. An earlier full run while the separate browser qualification ran concurrently timed out in two existing creation UI polls; standalone and final sequential reruns passed. No test timeouts were relaxed.
- API tests discard source storage before empty/populated recovery; compare complete logical values and exact source metadata/proof/records after reopen. Adversarial cases cover wrong passwords, missing metadata/proof, unsupported version/KDF, changed salts/nonces/ciphertext, truncation, duplicate/mismatched IDs, damaged later records, invalid logical content, input/count limits, existing-ID rejection, duplicate names, schema quota failure, final transaction write failure, acknowledged-but-lost durable writes, registry failure, uncertain publication readback, retry, and concurrent restores. Synthetic private markers are absent from backup/error text.
- Mounted real-page tests exercise encrypted Blob download, source-storage disposal, file selection, failed password/file validation, authenticated preview, confirmation, collision/cancel, storage failure without success/partial destination, empty retry, and reopen through the real workflow/collection.
- `pnpm run check`: passed, zero errors/warnings. Ignored Paraglide output was generated with `pnpm exec paraglide-js compile --project ./project.inlang --outdir ./src/lib/paraglide` during setup.
- `pnpm run build`: passed. Existing SQL WASM dynamic/static import and plugin timing warnings remain.
- `node scripts/check-offline.mjs`: passed all existing offline gates plus the new built-app recovery check in `scripts/check-backup.mjs`. It downloads empty/populated backups offline, closes the source browser context, restores into a separate clean context offline, reloads/unlocks, and compares the full metadata/proof/encrypted record set from a second download. No live browser tab or live data was used.
- Focused `pnpm exec biome format --write ...` and `pnpm exec biome lint src/lib/vault/backup.ts src/lib/vault/backup.test.ts src/lib/vault/crypto.ts src/lib/vault/registry.ts src/lib/vault/vault.ts src/routes/backup.test.ts scripts/check-backup.mjs scripts/check-offline.mjs`: passed without warnings.
- Svelte autofixer on `RestoreBackup.svelte` and `+page.svelte`, installed Svelte `5.56.8`: no issues or suggestions.
- `git diff --check`: passed.
- `just crap`: attempted; blocked in `sdk-build` by `ERR_PNPM_UNSAFE_TASK_RUN_STATE_PATH` because the required reused `node_modules` is a symlink. Rust feature-powerset tests were not run: no Rust/storage-library implementation changed; the spec's application validation boundary applies.

### Integration and scope

Root/app dependency symlinks match the other worktrees; no install, dependency, or lockfile changes. No shared map, parent spec, glossary map, migration, or source database edits. No push. Integration tip merge and final commit/ancestry are recorded by the completion report.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-04`, based on integration `implement/password-manager-parity` at `4e444a520`. Approved test seams: visible mounted application actions and public vault API with real SQL/IndexedDB persistence. Complexity: medium/high; risk: high (data loss, identity/authentication damage, plaintext exposure); priority: complete authenticated validation before writes and atomic destination publication. No shared map/spec edits or live browser tabs.
