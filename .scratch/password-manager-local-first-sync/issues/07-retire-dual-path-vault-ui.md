# 07: Migrate legacy synced data and retire the dual-path UI

**What to build:** Move vaults created through either the old local or old synced path into the unified selector and attachment lifecycle. Keep original data recoverable until the result is verified, then remove the separate synced-vault creation and discovery workflow and its obsolete implementation.

**Blocked by:** 06 — Merge independently populated local and remote databases with consent

**Status:** ready-for-agent

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Preserve existing local and legacy synced vault IDs, master passwords, encrypted records, and Resource bindings.
- Use a supported, non-destructive migration with recovery from interruption; do not clear browser storage or force credential rewrites.
- Remove obsolete code and documentation after all callers and fixtures use the unified flow. Do not leave parallel legacy guidance.
- Do not add cross-User sharing, key distribution, or full two-Device convergence qualification.

## Acceptance checks

- [ ] Fixtures from both old creation paths open in the unified selector with their existing IDs and passwords.
- [ ] Existing synced data retains its Resource binding and resumes authorized sync without creating a duplicate Resource.
- [ ] Interrupted migration can be retried without data loss, duplicate vaults, or an unrecoverable partial state.
- [ ] Original data remains recoverable until the unified copy and decryption are verified.
- [ ] The UI exposes one Create vault path and no separate synced-vault discovery or unlock workflow.
- [ ] Obsolete dual-path implementation and tests are removed, current documentation matches the behavior, and the real-backend Password Manager suite passes.

## Current evidence and qualification limits

The legacy split remains necessary until existing users have a verified path into the unified database and sync lifecycle. Completion of this ticket establishes one active Password Manager database per browser profile, not a two-Device or sharing qualification.
