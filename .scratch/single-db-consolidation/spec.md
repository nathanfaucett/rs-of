Status: ready-for-agent

## Problem Statement

The password manager uses a separate IndexedDB database per vault (`password-manager-local-${uuid}`). This fragments storage, complicates quota management, and prevents atomic cross-vault operations. Consolidate to a **single shared IndexedDB database** with vault-prefixed tables.

## Solution

**Single IndexedDB database** (`password-manager-v1`) with tables prefixed by vault ID:

| Old (per-vault DB) | New (shared DB) |
|-------------------|-----------------|
| `vault_meta` | `vault_{uuid}_meta` |
| `secrets` | `vault_{uuid}_secrets` |
| `folders` | `vault_{uuid}_folders` |
| (lock_settings in meta) | `vault_{uuid}_lock_settings` |

Registry (`password-manager-vault-registry-v1`) remains separate — tracks vault metadata only.

Three changes:

1. **Shared database layer** — `SharedDb.ts`: open/manage single DB, provide prefixed table access
2. **VaultHandle refactor** — Use `SharedDb` with prefixed table names; drop `databaseName` field
3. **Migration on unlock** — Detect old per-vault DBs, copy data to shared DB, delete old DBs

## User Stories

1. As a user with multiple vaults, I want all data in one origin so quota is shared and predictable.
2. As a user, I want migration to happen automatically on unlock — no manual steps.
3. As a developer, I want simpler backup/restore (one DB to export/import).
4. As a developer, I want cross-vault operations (future: search across vaults) to be possible.

## Implementation Decisions

### Architecture

| Aspect | Decision |
|--------|----------|
| **Shared DB name** | `password-manager-v1` (bumped from v1 to force migration) |
| **Table prefix** | `vault_{uuid}_` — sanitized UUID (no hyphens) |
| **Registry** | Unchanged — still separate IndexedDB for vault list |
| **Migration trigger** | On `unlockVault()` — detect old DB, migrate, delete |
| **Migration strategy** | Copy tables → verify → delete old DB → update registry |
| **Concurrency** | Single DB connection shared; `SqlDatabase` handles locking |

### Components to Create

1. **`src/lib/vault/SharedDb.ts`** — Singleton managing shared DB connection:
   - `getInstance(): SharedDb`
   - `getClient(vaultId: string): SqlClient` — returns client with prefixed table context
   - `migrateVault(oldDbName: string, vaultId: string): Promise<void>`
   - `deleteVaultTables(vaultId: string): Promise<void>`

### Components to Modify

1. **`vault.ts`** — `VaultHandle`:
   - Remove `databaseName` field
   - Constructor takes `vaultId: string` + `SharedDb` client
   - All SQL uses prefixed tables: `vault_{id}_secrets`, etc.
   - `currentStorage()` → verify vault still exists in registry (no DB switch)

2. **`createVault()`** — Use `SharedDb`; create prefixed tables via migrations
3. **`unlockVault()`** — Before opening: check for old per-vault DB → migrate if exists
4. **`registry.ts`** — `VaultIdentity` drops `databaseName`/`previousDatabaseName`; add `migrated: boolean`

### Migration Logic (`SharedDb.migrateVault`)

```typescript
async migrateVault(oldDbName: string, vaultId: string): Promise<void> {
  const oldDb = await SqlDatabase.open(oldDbName);
  const oldClient = oldDb.client();
  
  // Read all tables
  const meta = await oldClient.execute(`SELECT * FROM vault_meta WHERE id = 1`);
  const secrets = await oldClient.execute(`SELECT * FROM secrets`);
  const folders = await oldClient.execute(`SELECT * FROM folders`);
  const lockSettings = await oldClient.execute(`SELECT lock_settings FROM vault_meta WHERE id = 1`);
  
  oldClient.close();
  oldDb.close();
  
  // Write to shared DB with prefixed tables
  const client = this.getClient(vaultId);
  await client.execute(`CREATE TABLE IF NOT EXISTS vault_${vaultId}_meta (...)`);
  // ... insert meta, secrets, folders, lock_settings
  
  // Verify counts match
  // Delete old DB: await indexedDB.deleteDatabase(oldDbName)
  // Update registry: migrated = true
}
```

### Table Schemas (Shared DB)

```sql
-- Per vault (vault_{uuid}_ prefix)
CREATE TABLE vault_{uuid}_meta (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  vault_id TEXT NOT NULL,
  salt TEXT NOT NULL,
  iv TEXT NOT NULL,
  created_at TEXT NOT NULL,
  authentication TEXT,
  item_format INTEGER NOT NULL DEFAULT 1,
  transition_backup TEXT,
  lock_settings TEXT
);

CREATE TABLE vault_{uuid}_secrets (
  id TEXT PRIMARY KEY,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE vault_{uuid}_folders (
  id TEXT PRIMARY KEY,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

## Testing Decisions

### Unit Tests

- `SharedDb`: singleton, prefixed table naming, migration copies all rows, deletes old DB
- `VaultHandle`: uses prefixed tables, no `databaseName` field
- `createVault`: creates tables in shared DB
- `unlockVault`: triggers migration when old DB exists

### Integration Tests

- Fresh install: vault created in shared DB, no old DBs
- Existing user: unlock vault → migration runs → old DB gone → data intact
- Multiple vaults: all in shared DB, independent encryption
- Delete vault: removes prefixed tables, registry entry

### Migration Test Matrix

| Scenario | Expected |
|----------|----------|
| No old DBs | No migration, create in shared DB |
| One old DB (v1-v5 migrations) | Migrate all tables, delete old DB |
| Multiple old DBs | Migrate each on unlock |
| Migration interrupted | Idempotent — re-run on next unlock |
| Corrupt old DB | Error, preserve old DB, user recovers from backup |

## Out of Scope

- Cross-vault search/queries (future)
- Sync protocol changes (separate spec)
- Registry consolidation (keep separate for vault list UI)

## Further Notes

- **Breaking change**: Requires migration for all existing users
- **Rollback**: Keep old DB until migration verified; user can recover from backup
- **Quota**: Single DB shares origin quota — monitor in production
- **WASM**: No changes needed — `SqlDatabase.open("password-manager-v1")` once
