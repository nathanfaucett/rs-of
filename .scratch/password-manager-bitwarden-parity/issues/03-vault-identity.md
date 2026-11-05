# 03: Separate vault identity from its name

**What to build:** Local vault creation, selection, and reopen using stable identities so display-name normalization cannot overwrite or merge vaults.

**Blocked by:** None (can start immediately).

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Use stable vault identity separate from display name through storage and the visible selector. Do not use a normalized display name as the sole identity.
- [x] Handle case, punctuation, whitespace, and duplicate-name collisions explicitly. Creation must never overwrite an existing vault; selection must identify the intended vault without ambiguity.
- [x] Preserve access to existing named vaults through a safe transition. Failed transitions leave the source usable; remove obsolete naming paths once replaced.
- [x] Handle concurrent creation or interrupted registration without silently adopting or replacing another vault. Show actionable local-storage errors.
- [x] Test user-visible create/select/unlock flows and real-persistence close/reopen for colliding names and isolated records. Include storage failure and existing-vault transition cases.
- [x] Keep this a local-vault concern, not an IdP User, Installation, Node, or organization model.

## Answer

Implemented in `1e3012260` on `implement/password-manager-03`.

- The local registry reserves a stable ID and a unique database name before creation writes vault data. Display names are trimmed labels. Duplicate names are allowed; the selector shows full IDs.
- Unlock resolves the selected ID and checks the stored metadata ID. Removed `vaultExists` and the old name-based create/unlock paths.
- Existing v1 vaults register in place with their stored IDs. Failed registration leaves the source unchanged and can be retried. No salt, records, or migration history are rewritten. Original spelling cannot be recovered from old normalized names.
- Interrupted registrations stay visible. Unlock recovers saved metadata; an incomplete reservation gives a recovery error and never adopts a later same-name vault. Creation and registration races cannot replace another identity.
- Added real SQL/IndexedDB/Web Crypto tests for collisions, duplicate names, concurrent creation/registration, repeated-ID rejection, metadata-ID mismatch, interrupted registration, and failed existing-vault registration. A mounted page test uses real persistence, workflow, and collection to create, select, lock, and reopen isolated records.

Validation:

- `vite build`: passed.
- `svelte-check --tsconfig ./tsconfig.json`: passed, no errors or warnings (run after build generated Paraglide files).
- `vitest run`: passed, 7 files / 36 tests, Chromium.
- Targeted `biome check` on changed source/tests: passed.
- Svelte autofixer on `+page.svelte`: no issues or suggestions.
- `git diff --check`: passed.
- Merged `implement/password-manager-parity` into this branch after the implementation commit: already up to date at `42391ef3c`; ancestry check passed. The v1 migration file is unchanged.

Overlap and limits:

- Ticket 02 also owns `vault.ts`. This ticket changed only identity resolution, registration, and identity checks there; password derivation and authentication logic are unchanged. Its callers/tests must use vault IDs, not names. Authentication work remains ticket 02.
- Test configuration resolves the SQL package path and uses a worktree-local cache. Root dependency symlinks were reused. Installed Vitest/browser version mismatch warnings remain; all tests passed.
- Rust validation and `just crap` were not run: no Rust or storage-library source changed. Cold offline launch and production security qualification are outside ticket 03.
- Shared map was not edited, as requested. No push or main checkout.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-03` in `.scratch/worktrees/password-manager-03`, based on `implement/password-manager-parity` at `42391ef3c`. Own registry, storage identity, selector/create UI, and their tests. Preserve the v1 schema migration. Ticket 02 owns authentication; avoid changes to that logic.
- Test seams: approved spec boundaries: mounted create/select/unlock flows and public vault API with real close/reopen persistence. Complexity: medium; risk: high (vault access/data loss); priority: collision safety and recoverable registration.
