# Ticket 06: Cleanup, Migration & Testing

## Type: task
## Status: open

### Description
Remove deprecated code, add comprehensive tests, verify migration path for existing users.

### Acceptance Criteria
- [ ] Remove `src/lib/vault/storage/SharedDb.ts` (no longer used)
- [ ] Remove direct `@ofsdk/sql` imports from `vault.ts` (keep only types/migrations)
- [ ] Update `package.json`: remove `@ofsdk/sql` from dependencies (or keep as peer for types)
- [ ] Add migration logic for existing local-only vaults:
  - On first unlock after update, create sync resource for existing vault
  - Upload local data to Storage Service via initial sync
  - Preserve vault ID, encryption key, all data
- [ ] Unit Tests:
  - `SyncVaultHandle` encryption/decryption round-trip
  - `SyncVaultHandle` change event mapping (remote-upsert → decrypted item)
  - `SyncClient` lifecycle (init, logout cleanup)
  - `OidcAuth` token refresh, callback handling
- [ ] Integration Tests:
  - Full flow: login → create vault → add secret → sync → verify in Storage Service
  - Two clients: login both → sync vault → concurrent edits → conflict handling
  - Offline: disconnect network → edit → reconnect → verify catch-up sync
  - Migration: existing local vault → unlock after update → sync works
- [ ] E2E Tests (Playwright):
  - Login flow
  - Vault create/unlock with sync indicator
  - Multi-device sync (two browser contexts)
  - Sync to device UI
- [ ] Performance: Sync startup < 2s, change propagation < 500ms LAN

### Implementation Notes
- Migration strategy:
  1. Detect existing vault in IndexedDB (old `password-manager-{vaultId}` DBs)
  2. Call `sync.sql.create("vault-" + vaultId)` with same name
  3. Open local WASM DB with new resourceId
  4. Run initial sync (uploads all local data)
  5. Delete old per-vault DBs after successful sync
- Keep `@ofsdk/sql` as devDependency for types/migrations if needed
- Run `pnpm test` and `pnpm check` before merge

### Dependencies
- Blocked by: 03, 04, 05 (all features complete)

### Testing
- All tests pass in CI
- Manual verification on staging environment
