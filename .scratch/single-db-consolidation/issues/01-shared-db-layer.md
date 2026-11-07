## Ticket 01: Shared Database Layer

**Blocked by**: none

### What it delivers
`SharedDb.ts` singleton managing the single shared IndexedDB connection with vault-prefixed table access.

### Scope
- Create `src/lib/vault/SharedDb.ts`
- Single `SqlDatabase` instance for `password-manager-v1`
- `getClient(vaultId)` returns `SqlClient` — caller uses prefixed table names
- `migrateVault(oldDbName, vaultId)` copies all tables, verifies, deletes old DB
- `deleteVaultTables(vaultId)` drops all 4 prefixed tables
- Table prefix: `vault_${sanitizedUuid}_` (UUID without hyphens)

### Acceptance
- Unit tests: singleton behavior, prefix format, migration copies rows, deletes old DB
- Integration: open shared DB, create tables for vault, query via prefixed names
