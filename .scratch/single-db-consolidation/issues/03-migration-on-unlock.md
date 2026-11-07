## Ticket 03: Migration on Unlock

**Blocked by**: 01, 02

### What it delivers
Automatic migration from old per-vault databases to shared DB on `unlockVault()`.

### Scope
- Modify `unlockVault()` in `vault.ts`:
  - Before opening shared DB: check if old DB exists (`password-manager-local-${vaultId}`)
  - If exists: call `SharedDb.migrateVault(oldDbName, vaultId)`
  - Migration: copy `vault_meta`, `secrets`, `folders`, `lock_settings` to prefixed tables
  - Verify row counts match
  - Delete old DB: `indexedDB.deleteDatabase(oldDbName)`
  - Update registry: set `migrated: true` on `VaultIdentity`
- Modify `registry.ts`:
  - `VaultIdentity`: remove `databaseName`, `previousDatabaseName`; add `migrated: boolean`
  - `listVaultIdentities()`: detect old DBs by prefix, mark `migrated: false`
  - `completeRegistration()`: no `databaseName` needed
- Migration is idempotent — safe to re-run if interrupted

### Acceptance
- Integration test: existing vault (old DB) → unlock → migration runs → old DB gone → data intact
- Multiple vaults: each migrates independently on unlock
- Interrupted migration: re-runs on next unlock
- Fresh install: no migration, creates in shared DB directly
