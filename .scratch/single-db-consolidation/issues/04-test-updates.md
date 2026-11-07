## Ticket 04: Test Updates

**Blocked by**: 01, 02, 03

### What it delivers
All existing tests pass with single shared DB architecture.

### Scope
- Update test fixtures/helpers to use `SharedDb` (shared across tests or per-test cleanup)
- `vault.test.ts`: create/unlock use shared DB, verify prefixed tables
- `authentication.test.ts`: unlock triggers migration for old test DBs
- `backup.test.ts`: export/import uses shared DB
- `folders.test.ts`: folder operations use prefixed tables
- `lock.test.ts`: lock settings in shared DB
- `item-transition.test.ts`: transitions work with shared DB
- Cleanup: `afterEach` deletes vault tables via `SharedDb.deleteVaultTables(vaultId)`
- Registry tests: `VaultIdentity` without `databaseName`, with `migrated` flag

### Acceptance
- `cargo hack test --feature-powerset --all-targets` passes (or equivalent)
- No test depends on per-vault database names
- Migration tests cover: fresh, existing v1-v5, multiple vaults, interruption
