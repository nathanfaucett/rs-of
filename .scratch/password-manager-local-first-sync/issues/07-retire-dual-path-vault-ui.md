# 07: Migrate legacy synced data and retire the dual-path UI

**What to build:** Move vaults created through either the old local or old synced path into the unified selector and attachment lifecycle. Keep original data recoverable until the result is verified, then remove the separate synced-vault creation and discovery workflow and its obsolete implementation.

**Blocked by:** 06 — Merge independently populated local and remote databases with consent

**Status:** ready-for-agent

Work status: resolved

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Preserve existing local and legacy synced vault IDs, master passwords, encrypted records, and Resource bindings.
- Use a supported, non-destructive migration with recovery from interruption; do not clear browser storage or force credential rewrites.
- Remove obsolete code and documentation after all callers and fixtures use the unified flow. Do not leave parallel legacy guidance.
- Do not add cross-User sharing, key distribution, or full two-Device convergence qualification.

## Acceptance checks

- [x] Fixtures from both old creation paths open in the unified selector with their existing IDs and passwords.
- [x] Existing synced data retains its Resource binding and resumes authorized sync without creating a duplicate Resource.
- [x] Interrupted migration can be retried without data loss, duplicate vaults, or an unrecoverable partial state.
- [x] Original data remains recoverable until the unified copy and decryption are verified.
- [x] The UI exposes one Create vault path and no separate synced-vault discovery or unlock workflow.
- [x] Obsolete dual-path implementation and tests are removed, current documentation matches the behavior, and the real-backend Password Manager suite passes.

## Current evidence and qualification limits

Merged in `1942a064`, with the backup recovery fix in `e6936354`. Historical local and synced fixtures reopen through the unified selector with their original passwords. The real-backend tests covered exact Resource reuse, interrupted upgrade and retry, retained source recovery, and rejection of an incompatible source before retirement. The seeded Password Manager project and focused Create and Join flows passed; the final backup fix passed focused recovery tests. The source is retained read-only in supported upgrades. Historical per-vault local data copies live records; untranslated history remains in the original source. The full browser matrix and two-Device convergence were not qualified.
