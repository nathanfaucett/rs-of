# 01: Share one encrypted vault implementation across local and synced workflows

**What to build:** Make the existing local and legacy synced workflows use one encrypted vault implementation over their respective database handles. Preserve current behavior while removing divergent CRUD, authentication, lock-setting, and ciphertext handling so later attachment work has one vault seam.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Keep vault IDs, master passwords, schemas, lock settings, and existing Storage Resource bindings unchanged.
- Preserve the current local and legacy synced UI paths until the unified attachment and migration paths are complete.
- Do not change Storage registration, transport, or automatic synchronization in this ticket.

## Acceptance checks

- [ ] Signed-out local create, unlock, edit, and lock behavior remains unchanged.
- [ ] Signed-in legacy synced create, unlock, edit, and lock behavior remains unchanged against a real backend.
- [ ] Existing vault IDs, passwords, lock settings, and Resource bindings survive reopen without migration or duplication.
- [ ] The shared implementation preserves complete encrypted records, including ciphertext and nonce, and never persists plaintext secrets through the new path.
- [ ] Focused vault tests and the existing real-backend Password Manager workflow pass.

## Current evidence and qualification limits

Local and synced vaults currently have separate implementations and browser databases. This ticket is a compatibility-preserving prefactor; it does not deliver first attachment or merge.
