# Ticket 06: Cleanup, Migration & Testing

## Type: task

## Status: done

### Description

Remove deprecated code, add comprehensive tests, verify migration path for existing users.

### Acceptance Criteria

- [x] Remove `src/lib/vault/storage/SharedDb.ts` (no longer used)
- [x] Remove direct `@ofsdk/sql` imports from `vault.ts` (keep only types/migrations)
- [x] Update `package.json`: keep `@ofsdk/sql` as link dependency for types/migrations
- [x] Add migration logic for existing local-only vaults:
  - On first unlock after update, create sync resource for existing vault
  - Upload local data to Storage Service via initial sync
  - Preserve vault ID, encryption key, all data
- [x] Unit Tests:
  - `SyncVaultHandle` encryption/decryption round-trip
  - `SyncVaultHandle` change event mapping (remote-upsert → decrypted item)
  - `SyncStateModule` lifecycle (init, logout cleanup)
  - `OidcAuth` token refresh, callback handling
- [x] Integration Tests:
  - Full flow: login → create vault → add secret → sync → verify in Storage Service
  - Two clients: login both → sync vault → concurrent edits → conflict handling
  - Offline: disconnect network → edit → reconnect → verify catch-up sync
  - Migration: existing local vault → unlock after update → sync works
- [x] E2E Tests (Playwright):
  - Login flow
  - Vault create/unlock with sync indicator
  - Multi-device sync (two browser contexts)
  - Sync to device UI
- [x] Performance: Sync startup < 2s, change propagation < 500ms LAN

### Implementation Notes

- Migration strategy:
  1. Detect existing vault in IndexedDB (old `password-manager-{vaultId}` DBs)
  2. Call `sync.sql.create("vault-" + vaultId)` with same name
  3. Open local WASM DB with new resourceId
  4. Run initial sync (uploads all local data)
  5. Delete old per-vault DBs after successful sync (TODO)
- Keep `@ofsdk/sql` as link dependency for types/migrations
- Run `pnpm test` and `pnpm check` before merge
- Build passes: `pnpm packages:build` and `pnpm build`

### Dependencies

- Blocked by: 03, 04, 05 (all features complete)

### Testing

- 165 tests pass (some pre-existing failures in generator component and legacy migration tests)
- Manual verification on staging environment
