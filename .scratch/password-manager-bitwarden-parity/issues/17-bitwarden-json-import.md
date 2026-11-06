# 17: Preview and atomically import Bitwarden JSON

**What to build:** Import the verified Bitwarden JSON subset into a chosen local vault after an informative preview, without silent loss or partial writes.

**Blocked by:** 07 — Store Secure Note items; 08 — Store Card items; 09 — Store Identity items; 10 — Store SSH Key items; 12 — Organize items in encrypted folders; 13 — Recover deleted items through Trash.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Document and implement the evidence-backed supported JSON subset across all five item types, common metadata, folders, history, URI settings, and custom fields. Use synthetic fixtures, never live credential exports.
- [x] Preview counts, validation problems, duplicates, and unsupported data before any writes. Unknown field variants, attachments, passkeys, organization ownership, shared permissions, and other unmapped data must be reported, not silently dropped or converted.
- [x] Provide explicit duplicate behavior for repeated imports. Do not overwrite by display name. Unsupported data requires a clear rejection or explicitly acknowledged exclusion, with the original input unchanged.
- [x] Validate the entire import before commit. Use an atomic persistence boundary so failed/quota-limited/interrupted imports leave the destination unchanged. Replace the single-item limitation only as needed; do not claim an ordinary write loop is atomic.
- [x] On success, show imported items in the real list/detail/folder/Trash flows and verify them after close/reopen. Keep source organization metadata distinct from actual local access rights.
- [x] Test Unicode, multiline text, missing optional values, every supported mapping, unknown variants, duplicates, cancelled preview, corrupt files, transaction failure, lock during import, and uncertain outcomes.
- [x] Extend encrypted backup/clean-storage restore coverage for imported data. Preserve offline use and do not put input contents in URLs, logs, telemetry, or notifications. CSV, other vendors, and encrypted Bitwarden formats are not implied support.

## Answer

Implemented the strict personal-json-v1 importer in `apps/password-manager/src/lib/import/`, mounted by the route hook in `src/routes/+page.svelte`. Operational details and bounds: `docs/password-manager-import.md`. No exporter, shared fixture, map, or spec changes.

### Acceptance evidence

- All five mappings: parser tests verify Login credentials/TOTP, all seven URI states, ordered custom variants and duplicate names, complete Card/Identity/SSH properties, Note subtype/body, Unicode/multiline values, leading zeroes, metadata, ordered history, fresh IDs and remapped folders. RFC 3339 validation rejects impossible dates and preserves submillisecond precision while normalizing timezone offsets. Missing creation/revision dates have path-only preview warnings.
- Preview/rejection: mounted page shows destination UUID, type/folder counts, validation and unsupported JSON Pointer paths, and overlapping source-ID/logical-duplicate counts. All shared accept/block cases run; blocked cases also exercise the public import API and compare unchanged authoritative storage. Unknown nested URI/custom/type fields, permissions, attachments, passkeys, ownership, archive, deletion, reprompt, variants, corrupt/trailing JSON, escaped duplicate members, size/count/depth limits block the whole batch. No imported values enter diagnostics.
- Duplicate policy: default cancel or explicit import-as-new for every item/folder. Source provenance is encrypted and survives close/reopen and recovery. No-ID repeats require confirmation; identical no-ID entries within one file stay separate. Neither display names nor source IDs authorize overwrite. Stale previews and concurrent confirmations reject rather than silently repeat.
- Atomicity: one `mutateItems` invocation, asserted through the public real-persistence boundary. No write loop or storage implementation change. Real IndexedDB quota failures, lost SQL persistence, rejected registry publication and failed post-publication readback prove old-or-complete-new visibility, never partial publication or false success. A used preview cannot retry an uncertain write. Lock/reopen/verify guidance is explicit.
- Workflow: real page/workflow/collection/SQL tests cover cancel with unchanged storage, success in the list, protected detail/history, folder catalog, Trash/restore, close/reopen, dirty-navigation refusal, focus, duplicate choice and storage feedback. Editor subtree remount reloads the authoritative complete workflow/collection and folder catalog; no second item store is introduced.
- Locks: manual and automatic lock during file read, vault read, encryption, pre-write submission and post-commit acknowledgement retire preview and block late decrypted views/errors/success. A completed sent write is correctly present after reopen; a pre-submit retired handle writes nothing. Session deadline checks also guard completion.
- Recovery: mixed existing/imported data retains existing empty folders, Trash, ordered history and encrypted settings. Source storage is discarded; encrypted backup restore in clean storage compares exact logical items, encrypted item/folder records and settings, including durable provenance. Synthetic markers do not appear in encrypted records or the backup envelope.
- Offline regression: the built application check passes with network disabled, server stopped, new-browser-process startup and clean-profile encrypted recovery. This existing script qualifies the unchanged core workflows; it is not a separate built-app import scenario.

### Validation (2026-11-05)

All browser/test/build/check commands used `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`. Vitest used `--maxWorkers=1 --no-file-parallelism`.

- Focused parser/API/UI red → green cycles passed; **61 import tests** in the final full run.
- `pnpm test --maxWorkers=1 --no-file-parallelism`: **31 files, 347 tests passed** (492.05 s).
- `pnpm run build`: passed. Existing SQL static/dynamic import warning remains.
- `pnpm run check`: **0 errors, 0 warnings**.
- `node scripts/check-offline.mjs`: passed all existing built-app/offline/recovery checks.
- Svelte autofixer: both new import component and route hook returned no issues or suggestions.
- `git diff --check`: passed.

Worktree setup required repairing only the ignored local SQL dependency symlink, generating the local headless package output, and generating Paraglide output through the app build. No dependency/lockfile changes or generated artifacts are committed. One UI validation attempt timed out waiting for ticket 18's shared lock; the later serialized run passed.

### Limits and handoff

Limits: 16 MiB UTF-8, 10,000 items, 10,000 folders, 64 nesting levels. Whitespace-only source IDs reject because the integrated encrypted provenance model requires nonblank IDs (narrower than merely nonempty external IDs). Unknown data blocks rather than being silently excluded. Unsupported formats/features stay unsupported as documented. Atomic publication acknowledgement can be uncertain; sent writes are not rolled back by lock. Existing encrypted staging-database retention consumes space.

Exporter 18 remains independent. Production exporter round-trip and selected Bitwarden/Vaultwarden acceptance are not claimed here. No live browser/account/data, full Rust/monorepo checks, secure-erasure claim, or independent security review. The map/spec remain unchanged as requested. No push or root-branch merge.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-17`, based on integration `5b5dc9564`. Scope: import module, import controls/tests, and route hook only; exporter, shared fixtures, map, and spec stay unchanged. Use the approved mounted-page/public real-persistence boundaries plus focused parser tests. Complexity: high (strict external mapping); risk: data loss, plaintext disclosure, stale async results; priority: complete validation and one atomic batch before real-data use.
