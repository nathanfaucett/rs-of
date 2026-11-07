Status: ready-for-agent

## Problem Statement

The password manager currently uses **per-vault prefixed tables** in a shared SQLite database (`vault_{uuid}_meta`, `vault_{uuid}_secrets`, `vault_{uuid}_folders`) plus a separate **registry IndexedDB** (`password-manager-vault-registry-v2`) for vault listing. This adds complexity:

- Dynamic SQL with table name interpolation (`${tablePrefix(vaultId)}secrets`)
- Two databases to keep in sync (registry + shared DB)
- `pending`/`ready` status for two-phase vault creation
- Complex migration logic (`migrateVault` copies between DBs)
- No cross-vault queries possible
- Schema changes require per-vault migrations

## Solution

**Single shared tables with `vault_id` ownership columns** (matching Bitwarden/Vaultwarden architecture):

| Table           | Ownership | Key Columns                                                                    |
| --------------- | --------- | ------------------------------------------------------------------------------ |
| `vaults`        | Registry  | `id` (PK), `user_id`, `name`, `salt`, `iv`, `created_at`, `status`, `migrated` |
| `secrets`       | Per-item  | `id` (PK), `vault_id` FK, `ciphertext`, `nonce`, `created_at`, `updated_at`    |
| `folders`       | Per-item  | `id` (PK), `vault_id` FK, `ciphertext`, `nonce`, `created_at`, `updated_at`    |
| `lock_settings` | Per-vault | `vault_id` (PK), `ciphertext`, `nonce`, `updated_at`                           |

**Eliminate**: Registry IndexedDB, per-vault table prefixes, `pending` status, dynamic SQL.

## User Stories

1. **As a user** with multiple vaults, I want simpler backup/restore (one DB file).
2. **As a user**, I want vault creation to be atomic — no "interrupted registration" state.
3. **As a developer**, I want standard SQL with static table names — no dynamic queries.
4. **As a developer**, I want cross-vault search/queries to be possible in future.
5. **As a developer**, I want single migration path for all vaults.

## Architecture

| Aspect                | Decision                                                                                                                   |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| **Shared DB name**    | `password-manager-v1` (same as current shared DB)                                                                          |
| **Table strategy**    | Shared tables with `vault_id` FK columns                                                                                   |
| **Registry**          | Merged into `vaults` table in shared DB                                                                                    |
| **Migration trigger** | One-time on first unlock after upgrade                                                                                     |
| **Vault creation**    | Single transaction: `vaults` + empty tables                                                                                |
| **Vault deletion**    | `DELETE FROM ... WHERE vault_id = ?`                                                                                       |
| **Crypto isolation**  | Per-vault key derived from `vaults.salt`/`iv` — same as today                                                              |
| **Query API**         | **Typed `Client.execute()` / `Transaction.execute()`** with Valibot parsers (replaces manual `executeRaw` + index parsing) |

## Components to Create

1. **`src/lib/vault/schema.ts`** — Shared table DDL and migration list
2. **`src/lib/vault/VaultRegistry.ts`** — Replaces `registry.ts`; queries `vaults` table
3. **`src/lib/vault/SharedDb.ts`** — Simplified: no prefixed tables, no migration logic

## Components to Modify

1. **`vault.ts`** — `VaultHandle`:
   - Remove `tablePrefix()`, `prefixed()` helpers
   - **All queries use `Client.select()` / `Transaction.select()` with Valibot parsers**
   - Static table names + `WHERE vault_id = ?` parameters
   - Constructor takes `vaultId` + shared `SqlClient`
   - `createVault()` → single transaction insert into `vaults`
   - `unlockVault()` → read from `vaults` table, no legacy DB check

2. **`VaultRegistry.ts`** — All queries use typed `select()`
3. **`crypto.ts`** — `verifyAuthentication()` uses typed `select()` from `vaults` table
4. **`lock.ts`** — `loadLockSettings`/`saveLockSettings` use typed `select()` on `lock_settings` table
5. **`registry.ts`** — **Delete** (replaced by `VaultRegistry.ts`)

## Schema (Shared DB)

```sql
-- Vault registry (replaces separate IndexedDB)
CREATE TABLE vaults (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  salt TEXT NOT NULL,        -- base64, 16 bytes
  iv TEXT NOT NULL,          -- base64, 12 bytes
  created_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'ready',  -- 'ready' only (no 'pending')
  migrated INTEGER NOT NULL DEFAULT 1,   -- legacy migration flag
  -- lock_settings moved to separate table
);

-- Per-vault lock settings (encrypted with vault key)
CREATE TABLE lock_settings (
  vault_id TEXT PRIMARY KEY REFERENCES vaults(id) ON DELETE CASCADE,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Secrets (passwords, notes, cards, etc.)
CREATE TABLE secrets (
  id TEXT PRIMARY KEY,
  vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_secrets_vault ON secrets(vault_id);

-- Folders
CREATE TABLE folders (
  id TEXT PRIMARY KEY,
  vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_folders_vault ON folders(vault_id);
```

## Typed SQL API Usage

The migration replaces all manual `executeRaw()` + index-based parsing with **typed `execute()`** from `@ofsdk/sql`:

```typescript
import { Client, Transaction, Row, ExecuteError } from "@ofsdk/sql";
import * as v from "valibot";

// Valibot schema for a secret row
const SecretRowSchema = v.object({
  id: v.string(),
  ciphertext: v.string(),
  nonce: v.string(),
  createdAt: v.string(),
  updatedAt: v.string(),
});

// In VaultHandle.loadSecrets():
async loadSecrets(): Promise<Secret[]> {
  return this.client.execute(
    `SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
    (row: Row) => SecretRowSchema.parse({
      id: row.id.value,
      ciphertext: row.ciphertext.value,
      nonce: row.nonce.value,
      createdAt: row.created_at.value,
      updatedAt: row.updated_at.value,
    }),
    [{ type: "text", value: this.vaultId }],
  );
}

// In VaultRegistry.listVaultIdentities():
async listVaultIdentities(): Promise<VaultIdentity[]> {
  return this.client.execute(
    `SELECT id, name, status, migrated FROM vaults`,
    (row: Row) => ({
      id: row.id.value,
      name: row.name.value,
      status: row.status.value as "ready",
      migrated: row.migrated.value === "1",
    }),
  );
}
```

**Benefits:**

- Parser runs **after** execution, receives null-prototype `Row` with tagged `DbValue` cells
- `ExecuteError` includes `reason: "parser"` and `rowIndex` on failure
- Sync parser — async rejected at runtime with structured error
- Empty result → empty array (parser never invoked)
- Duplicate columns / multiple statements rejected automatically

## Migration Logic (One-time)

```typescript
// Run once on first unlock after upgrade
async function migrateToSharedTables(): Promise<void> {
  const sharedDb = SharedDb.getInstance();
  const client = await sharedDb.getClient(); // no vaultId needed

  // 1. Create new tables
  await client.execute(CREATE_VAULTS_TABLE);
  await client.execute(CREATE_LOCK_SETTINGS_TABLE);
  await client.execute(CREATE_SECRETS_TABLE);
  await client.execute(CREATE_FOLDERS_TABLE);

  // 2. Migrate from registry IndexedDB
  const registryEntries = await oldRegistry.listVaultIdentities();
  for (const entry of registryEntries) {
    const vaultId = entry.id;
    const oldDbName = `password-manager-local-${vaultId}`;

    // Try open old per-vault DB
    const oldDb = await SqlDatabase.open(oldDbName);
    const oldClient = oldDb.client();

    // Read old tables
    const meta = await oldClient.execute(
      `SELECT * FROM vault_meta WHERE id = 1`,
    );
    const secrets = await oldClient.execute(`SELECT * FROM secrets`);
    const folders = await oldClient.execute(`SELECT * FROM folders`);
    const lockSettings = await oldClient.execute(
      `SELECT lock_settings FROM vault_meta WHERE id = 1`,
    );

    oldClient.close();
    oldDb.close();

    // Insert into shared tables
    await client.execute(
      `INSERT INTO vaults (id, name, salt, iv, created_at, status, migrated) VALUES (?, ?, ?, ?, ?, 'ready', 1)`,
      [meta.vault_id, meta.salt, meta.iv, meta.created_at],
    );

    for (const s of secrets) {
      await client.execute(
        `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
        [s.id, vaultId, s.ciphertext, s.nonce, s.created_at, s.updated_at],
      );
    }

    for (const f of folders) {
      await client.execute(
        `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
        [f.id, vaultId, f.ciphertext, f.nonce, f.created_at, f.updated_at],
      );
    }

    if (lockSettings[0]?.lock_settings) {
      await client.execute(
        `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
        [
          vaultId,
          lockSettings.ciphertext,
          lockSettings.nonce,
          new Date().toISOString(),
        ],
      );
    }

    // Delete old DB
    await indexedDB.deleteDatabase(oldDbName);
  }

  // 3. Delete registry IndexedDB
  await indexedDB.deleteDatabase("password-manager-vault-registry-v2");
}
```

## Testing Decisions

### Unit Tests

- `VaultRegistry`: CRUD on `vaults` table
- `VaultHandle`: static table queries with `vault_id` param
- `createVault`: single transaction inserts `vaults` row
- `unlockVault`: reads from `vaults` table
- Migration: copies all data, deletes old DBs

### Integration Tests

- Fresh install: vault created in shared tables, no registry DB
- Existing user: unlock → migration runs → registry DB gone → data intact
- Multiple vaults: all in shared tables, independent encryption keys
- Delete vault: cascades to secrets/folders/lock_settings

### Migration Test Matrix

| Scenario               | Expected                                          |
| ---------------------- | ------------------------------------------------- |
| Fresh install          | No migration, vaults table used directly          |
| One legacy vault       | Migrate all tables, delete old DB + registry DB   |
| Multiple legacy vaults | Migrate each, single transaction per vault        |
| Migration interrupted  | Idempotent — re-run on next unlock                |
| Corrupt legacy vault   | Error, preserve old DB, user recovers from backup |

## Out of Scope

- Cross-vault search/queries (enabled by schema, separate feature)
- Sync protocol changes
- Multi-user support (single-user app)

## Further Notes

- **Breaking change**: Requires migration for all existing users
- **Rollback**: Migration preserves old DBs until verified
- **Quota**: Single DB shares origin quota — simpler to monitor
- **WASM**: No changes — `SqlDatabase.open("password-manager-v1")` once
- **Registry UI**: `listVaults()` now queries `SELECT id, name, status FROM vaults`
- **Pending status removed**: Vault creation is atomic in single transaction
