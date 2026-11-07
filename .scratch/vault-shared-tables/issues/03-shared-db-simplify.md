Status: open

## Task

Simplify `SharedDb.ts` — remove prefixed table logic, migration, per-vault client caching.

## Details

**Remove:**
- `tablePrefix(vaultId)` function
- `initializedVaults` Set
- `ensureTables(vaultId, client)` — schema now in migrations
- `migrateVault(oldDbName, vaultId)` — moved to one-time migration script
- `deleteVaultTables(vaultId)` — replaced by `DELETE WHERE vault_id = ?`

**Keep:**
- `getInstance(): SharedDb` — singleton
- `getClient(): SqlClient` — **single shared client** (no vaultId param)
- `close(): void` — cleanup

**New:**
- `getClientForVault(vaultId: string): SqlClient` — returns same client, callers add `WHERE vault_id = ?`

## Acceptance Criteria

- [ ] Single `SqlClient` shared across all vaults
- [ ] No dynamic table names in `SharedDb`
- [ ] No per-vault initialization logic
- [ ] Vault-specific queries handled by callers (`VaultHandle`, `VaultRegistry`)

## Dependencies

Blocked by: 01
