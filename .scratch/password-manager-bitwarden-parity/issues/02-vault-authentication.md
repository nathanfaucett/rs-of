# 02: Authenticate empty and populated vaults

**What to build:** Consistent master-password authentication before a local vault becomes unlocked, even when it contains no items.

**Blocked by:** None (can start immediately).

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] New empty vaults accept the correct password and reject wrong passwords before showing an unlocked view. The same rules hold for populated vaults and vaults whose last item was removed.
- [x] Authenticate independently of item presence using a reviewed authenticated vault-level mechanism. Do not treat algorithm choice as proof of security.
- [x] Define safe handling of existing vaults without authentication metadata. A legacy empty vault has no stored password proof; do not claim to verify its original password or silently establish a new password from a failed attempt.
- [x] Preserve existing encrypted data and ensure failed authentication or metadata writes do not damage the vault. Provide actionable errors without logging passwords or keys.
- [x] Test the visible unlock flow and the public vault API with real persistence across create, close, and reopen. Cover correct/wrong passwords, empty/populated data, removal of the last item, malformed authentication data, and write failure.
- [x] Keep authentication entirely local. Remove obsolete authentication paths after the safe format transition; add no legacy writer or compatibility wrapper.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-02`, based on integration branch `implement/password-manager-parity` at `42391ef3c`. Preserve its v1 migration. Scope: vault authentication and its tests; ticket 03 owns vault identity. Use the approved visible application and real-persistence public vault API test boundaries. No shared map edits.

## Answer

Implemented in `apps/password-manager/src/lib/vault/vault.ts`. Migration 2 adds a nullable authentication field without changing migration 1 from `42391ef3c`. New vaults atomically insert metadata and a versioned AES-GCM proof with a fresh nonce. The proof binds its purpose/version and vault metadata. Unlock checks it before returning a handle, independent of records. No password, exported key, or decrypted item enters persistent metadata or error messages.

Existing v1 populated vaults must decrypt every record before writing a proof. The upgrade changes no encrypted record or derivation metadata and reads/verifies the saved proof before unlock. Wrong passwords, damaged records, and failed writes cannot establish an unlocked session. Missing-proof empty vaults stay locked for every attempted password. The visible error explains that the original password cannot be verified and directs the user to keep the source and create a separate vault. Malformed proofs never fall back to legacy authentication. The obsolete first-record-only unlock path is removed.

The app README records the mechanism, transition, write-outcome handling, and threat limits. This is not an independent security audit or a guarantee against rollback, deletion, offline password guessing, or a compromised browser.

### Validation

- Red: the new real-persistence empty-vault test failed because a wrong password returned a handle. Green: the full app suite passes, **7 files / 35 tests**, using Chromium, Web Crypto, real SQL/WASM, and IndexedDB.
- API tests cover empty/populated/last-item-removed vaults, failed-attempt data preservation, real v1 schema transition, unprovable legacy empty vaults, malformed/unsupported/truncated/tampered proofs, metadata binding, damaged legacy records, and real IndexedDB `put` failures during creation and upgrade. Retry/reopen preserves usable records.
- Mounted page tests use the real vault, workflow, and collection. Visible creation, lock, remount, wrong-password rejection, successful unlock, populated data, last-item removal, and legacy-empty guidance pass. No mock vault is used in these tests.
- `pnpm test`: passed, 35 tests.
- `pnpm run build`: passed; existing SQL WASM dynamic/static import warning.
- `pnpm run check`: passed, zero errors and warnings. Generated ignored Paraglide files with `pnpm exec paraglide-js compile --project ./project.inlang --outdir ./src/lib/paraglide` first.
- Focused `pnpm exec biome format --write ...` and `biome lint ...`: passed; one existing `listVaults` non-null assertion warning remains in ticket 03's identity area.
- Svelte autofixer on the unchanged `+page.svelte`: no issues or suggestions. No Svelte component change was needed.
- `git diff --check`: passed.
- `just crap`: blocked in its `sdk-build` prerequisite by `ERR_PNPM_UNSAFE_TASK_RUN_STATE_PATH`, because worktree `node_modules` is a symlink. No Rust/storage-library source changed; Rust feature-powerset tests were not run.

### Integration constraints

- Root/app dependencies are symlinked to the existing root checkout. No install or dependency change was needed. The test WASM allow-list now resolves the installed SQL package rather than assuming the worktree is beside `ofdb`.
- Keep migration 1 unchanged. Migration 2 belongs to authentication; later migrations must append new versions.
- Ticket 03 retains identity ownership. Authentication binds the persisted vault ID, salt, original metadata IV, and creation time. Do not change these fields without re-establishing the proof under the verified key. Display-name/catalog changes need not alter them.
- Future backup/restore must preserve the authentication proof and its bound metadata, or deliberately re-establish a proof under the verified key. The existing export is still not a complete backup.
- No shared map edits, push, or main checkout.
