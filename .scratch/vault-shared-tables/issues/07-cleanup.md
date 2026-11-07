Status: open

## Task

Delete obsolete files and clean up imports after migration.

## Details

### Files to Delete

1. **`src/lib/vault/registry.ts`** — Replaced by `VaultRegistry.ts`
2. **`src/lib/vault/SharedDb.ts`** (old version) — Replaced by simplified version
3. **`src/lib/vault/authentication-fixtures.ts`** — If only used by old registry tests

### Files to Update Imports

- `vault.ts` — Import `VaultRegistry` instead of `registry` functions
- `+page.svelte` — Import from `VaultRegistry`
- Test files — Update import paths

### Code to Remove from `vault.ts`

- `reserveVault` import → use `VaultRegistry.reserveVault`
- `completeRegistration` import → no-op (or remove call)
- `findVault` import → use `VaultRegistry.findVault`
- `listVaultIdentities` import → use `VaultRegistry.listVaultIdentities`
- Legacy DB migration logic in `unlockVault`
- `tablePrefix` function
- `sanitizeUuid` function (if only used for table prefixes)

## Acceptance Criteria

- [ ] `registry.ts` deleted
- [ ] Old `SharedDb.ts` replaced
- [ ] All imports updated
- [ ] No references to IndexedDB registry in vault code
- [ ] No dynamic table name logic
- [ ] `sanitizeUuid` removed if unused

## Dependencies

Blocked by: 01, 02, 03, 04, 05, 06
