Status: open

## Task

Create one-time migration script to move from per-vault tables + registry IndexedDB to shared tables using **typed `select()`** for reading legacy data.

## Details

Run on first unlock after upgrade. Located in `src/lib/vault/migrate-to-shared.ts`.

```typescript
import { Client, Row, Transaction } from "@ofsdk/sql";
import * as v from "valibot";
import { SecretRowSchema, FolderRowSchema } from "./schema";

export async function migrateToSharedTables(): Promise<void> {
  const sharedDb = SharedDb.getInstance();
  const client = await sharedDb.getClient();

  // 1. Run migrations to create new tables (vaults, lock_settings, secrets, folders)
  await client.migrate(MIGRATIONS);

  // 2. Check if migration already done
  const existing = await client.select(
    `SELECT COUNT(*) as count FROM vaults`,
    (row: Row) => ({ count: parseInt(row.count.value, 10) }),
  );
  if (existing[0]?.count > 0) return; // Already migrated

  // 3. Read registry IndexedDB
  const registryEntries = await oldRegistry.listVaultIdentities();

  for (const entry of registryEntries) {
    const vaultId = entry.id;
    const oldDbName = `password-manager-local-${vaultId}`;

    try {
      const oldDb = await SqlDatabase.open(oldDbName);
      const oldClient = oldDb.client();

      // Read old per-vault tables using typed select
      const metaResults = await oldClient.select(
        `SELECT vault_id, salt, iv, created_at, authentication, lock_settings FROM vault_meta WHERE id = 1`,
        (row: Row) => ({
          vaultId: row.vault_id.value,
          salt: row.salt.value,
          iv: row.iv.value,
          createdAt: row.created_at.value,
          authentication: row.authentication?.value ?? null,
          lockSettings: row.lock_settings?.value ?? null,
        }),
      );

      const secretsResults = await oldClient.select(
        `SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets`,
        (row: Row) =>
          SecretRowSchema.parse({
            id: row.id.value,
            ciphertext: row.ciphertext.value,
            nonce: row.nonce.value,
            createdAt: row.created_at.value,
            updatedAt: row.updated_at.value,
          }),
      );

      const foldersResults = await oldClient.select(
        `SELECT id, ciphertext, nonce, created_at, updated_at FROM folders`,
        (row: Row) =>
          FolderRowSchema.parse({
            id: row.id.value,
            ciphertext: row.ciphertext.value,
            nonce: row.nonce.value,
            createdAt: row.created_at.value,
            updatedAt: row.updated_at.value,
          }),
      );

      oldClient.close();
      oldDb.close();

      const meta = metaResults[0];
      if (!meta) continue;

      // Insert into shared tables (single transaction per vault)
      await client.transaction(async (tx: Transaction) => {
        await tx.execute(
          `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
          [
            meta.vaultId,
            entry.name,
            meta.salt,
            meta.iv,
            meta.createdAt,
            meta.authentication,
          ],
        );

        for (const s of secretsResults) {
          await tx.execute(
            `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
            [s.id, vaultId, s.ciphertext, s.nonce, s.createdAt, s.updatedAt],
          );
        }

        for (const f of foldersResults) {
          await tx.execute(
            `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
            [f.id, vaultId, f.ciphertext, f.nonce, f.createdAt, f.updatedAt],
          );
        }

        if (meta.lockSettings) {
          // lockSettings is encrypted JSON string { ciphertext, nonce }
          const parsed = JSON.parse(meta.lockSettings);
          await tx.execute(
            `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
            [
              vaultId,
              parsed.ciphertext,
              parsed.nonce,
              new Date().toISOString(),
            ],
          );
        }
      });

      // Delete old per-vault DB
      await indexedDB.deleteDatabase(oldDbName);
    } catch (e) {
      // Preserve old DB on error — user can recover from backup
      console.error(`Migration failed for vault ${vaultId}:`, e);
      throw e;
    }
  }

  // 4. Delete registry IndexedDB
  await indexedDB.deleteDatabase("password-manager-vault-registry-v2");
}
```

**Integration:** Call at start of `unlockVault()` if not yet migrated (check flag in localStorage or `vaults` table count).

## Acceptance Criteria

- [ ] Runs once per user (idempotent)
- [ ] Uses typed `select()` for reading legacy DBs
- [ ] Migrates all vaults from registry + per-vault DBs
- [ ] Preserves encryption keys (salt, iv, authentication)
- [ ] Preserves all secrets and folders
- [ ] Preserves lock settings
- [ ] Deletes old per-vault DBs after successful migration
- [ ] Deletes registry IndexedDB after all vaults migrated
- [ ] On error: preserves old DBs, throws for user recovery

## Dependencies

Blocked by: 01, 02, 03, 04
