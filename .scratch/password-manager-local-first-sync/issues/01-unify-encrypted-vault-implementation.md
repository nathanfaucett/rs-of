# 01: Share one encrypted vault implementation across local and synced workflows

**What to build:** Make the existing local and legacy synced workflows use one encrypted vault implementation over their respective database handles. Preserve current behavior while removing divergent CRUD, authentication, lock-setting, and ciphertext handling so later attachment work has one vault seam.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work status: resolved

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Keep vault IDs, master passwords, schemas, lock settings, and existing Storage Resource bindings unchanged.
- Preserve the current local and legacy synced UI paths until the unified attachment and migration paths are complete.
- Do not change Storage registration, transport, or automatic synchronization in this ticket.

## Acceptance checks

- [x] Signed-out local create, unlock, edit, and lock behavior remains unchanged.
- [x] Signed-in legacy synced create, unlock, edit, and lock behavior remains unchanged against a real backend.
- [x] Existing vault IDs, passwords, lock settings, and Resource bindings survive reopen without migration or duplication.
- [x] The shared implementation preserves complete encrypted records, including ciphertext and nonce, and never persists plaintext secrets through the new path.
- [x] Focused vault tests and the existing real-backend Password Manager workflow pass.

## Current evidence and qualification limits

Local and legacy synced vaults share the encrypted vault implementation. The existing database handles and formats are unchanged. Focused vault tests, 5 seeded Chromium backend tests, and all 27 Password Manager Playwright tests passed. This ticket does not deliver first attachment or merge.
