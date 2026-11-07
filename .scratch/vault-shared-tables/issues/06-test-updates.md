Status: open

## Task

Update tests for new shared-table architecture with **typed `execute()`** usage.

## Details

### Unit Tests to Update

1. **`vault.test.ts`** — `VaultHandle` tests
   - Mock `SharedDb.getClient()` returns shared client
   - Verify queries use typed `execute()` with Valibot parsers
   - Test `createVault` single transaction
   - Test `unlockVault` reads from `vaults` table with typed `execute()`

2. **`VaultRegistry.test.ts`** (replaces `registry.test.ts`)
   - Test against shared DB (not IndexedDB)
   - Verify `listVaultIdentities` uses typed `execute()` on `vaults` table
   - Verify `reserveVault` inserts into `vaults` atomically
   - Verify `deleteVault` cascades

3. **`SharedDb.test.ts`** — Simplify
   - Remove prefixed table tests
   - Test single shared client
   - Test migration script uses typed `execute()` (separate)

4. **Typed `execute()` parser tests**
   - Test parser success with valid rows
   - Test `ExecuteError` with `reason: "parser"` and `rowIndex` on invalid data
   - Test empty result returns empty array
   - Test duplicate column rejection
   - Test sync parser requirement (async throws)

### Integration Tests

1. **Fresh install flow**
   - Create vault → appears in selector → unlock works
   - No registry IndexedDB created

2. **Migration flow**
   - Set up old per-vault DBs + registry
   - Unlock vault → migration runs → data intact
   - Old DBs deleted, registry DB deleted

3. **Multi-vault**
   - Create 3 vaults
   - All in shared tables with different `vault_id`
   - Independent encryption keys
   - Delete one → cascades correctly

4. **Typed `execute()` integration**
   - Full query path: SQL → parser → typed result
   - Error handling: malformed row → `ExecuteError` with row index

### Test Helpers

- `createTestVault(client, vaultId, name, password)` — inserts into shared tables using `execute()`
- `createTestSecret(client, vaultId, ...)` — inserts secret
- `createTestFolder(client, vaultId, ...)` — inserts folder
- `parseTestRow(schema, row)` — helper for parser testing

## Acceptance Criteria

- [ ] All existing tests pass with new architecture
- [ ] Migration test covers success + failure cases
- [ ] No IndexedDB mocks needed for vault tests
- [ ] Test setup uses shared DB transactions
- [ ] Typed `execute()` parser tests cover error cases
- [ ] `ExecuteError.rowIndex` verified on parser failures

## Dependencies

Blocked by: 01, 02, 03, 04, 05
