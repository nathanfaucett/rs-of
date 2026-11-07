## Ticket 02: VaultHandle Refactor

**Blocked by**: 01

### What it delivers
`VaultHandle` uses `SharedDb` with prefixed tables; no per-vault `databaseName` field.

### Scope
- Modify `vault.ts`:
  - `VaultHandle` constructor: add `vaultId: string`, remove `databaseName`
  - Store `vaultId` and `SharedDb` client reference
  - All SQL queries use `vault_${vaultId}_meta`, `vault_${vaultId}_secrets`, `vault_${vaultId}_folders`
  - `currentStorage()`: verify vault exists in registry (no DB switch)
  - `close()`: release client reference (shared DB stays open)
- Update `createVault()`: use `SharedDb.getInstance().getClient(vaultId)`, run migrations on prefixed tables
- Update `unlockVault()`: get client from `SharedDb`, load meta from prefixed table

### Acceptance
- Unit tests: VaultHandle uses prefixed tables, no databaseName field
- Integration: create vault → tables in shared DB; unlock vault → loads from shared DB
- Multiple vaults: independent encryption, isolated tables
