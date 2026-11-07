import { Database as SqlDatabase, Client, type Row, Transaction } from "@ofsdk/sql";
import * as v from "valibot";
import { SharedDb } from "./SharedDb";
import { VaultRegistry, createVaultRegistry } from "./VaultRegistry";
import {
  parseSecretRow,
  parseFolderRow,
  parseVaultRow,
  type SecretRow,
  type FolderRow,
  MIGRATIONS,
  dbParams,
  dbParam,
  parseText,
  parseOptionalText,
} from "./schema";
import {
  decrypt,
  deriveKey,
  encrypt,
  fromBase64,
  toBase64,
  createAuthentication,
  verifyAuthentication,
} from "./crypto";
import { encryptLockSettings, decryptLockSettings, type LockSettings } from "../lock";

const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
const LEGACY_DB_PREFIX = "password-manager-local-";

interface LegacyVaultMeta {
  vaultId: string;
  salt: string;
  iv: string;
  createdAt: string;
  authentication: string | null;
  lockSettings: string | null;
}

/**
 * One-time migration from per-vault tables + registry IndexedDB to shared tables.
 * Runs on first unlock after upgrade. Idempotent - safe to re-run.
 */
export async function migrateToSharedTables(): Promise<void> {
  const sharedDb = SharedDb.getInstance();
  const client = await sharedDb.getClient();

  // 1. Run migrations to create new tables (vaults, lock_settings, secrets, folders)
  await client.migrate(MIGRATIONS);

  // 2. Check if migration already done
  const existing = await client.execute(
    `SELECT id FROM vaults LIMIT 1`,
    () => 1,
  );
  if (existing.length > 0) return; // Already migrated

  // 3. Read registry IndexedDB
  const registryEntries = await readRegistryIndexedDB();

  for (const entry of registryEntries) {
    const vaultId = entry.id;
    const oldDbName = `${LEGACY_DB_PREFIX}${vaultId}`;

    try {
      const oldDb = await SqlDatabase.open(oldDbName);
      const oldClient = oldDb.client();

      try {
        // Read old per-vault tables using typed execute
        const metaResults = await oldClient.execute(
          `SELECT vault_id, salt, iv, created_at, authentication, lock_settings FROM vault_meta WHERE id = 1`,
          (row: Row): LegacyVaultMeta => ({
            vaultId: parseText(row, "vault_id"),
            salt: parseText(row, "salt"),
            iv: parseText(row, "iv"),
            createdAt: parseText(row, "created_at"),
            authentication: parseOptionalText(row, "authentication"),
            lockSettings: parseOptionalText(row, "lock_settings"),
          }),
        );

        const secretsResults = await oldClient.execute(
          `SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets`,
          (row: Row) => ({
            id: parseText(row, "id"),
            ciphertext: parseText(row, "ciphertext"),
            nonce: parseText(row, "nonce"),
            createdAt: parseText(row, "created_at"),
            updatedAt: parseText(row, "updated_at"),
          }),
        );

        const foldersResults = await oldClient.execute(
          `SELECT id, ciphertext, nonce, created_at, updated_at FROM folders`,
          (row: Row) => ({
            id: parseText(row, "id"),
            ciphertext: parseText(row, "ciphertext"),
            nonce: parseText(row, "nonce"),
            createdAt: parseText(row, "created_at"),
            updatedAt: parseText(row, "updated_at"),
          }),
        );

        const meta = metaResults[0];
        if (!meta) {
          console.warn(`No metadata found for vault ${vaultId}, skipping`);
          continue;
        }

        // Insert into shared tables (single transaction per vault)
        const tx = await client.transaction();
        try {
          const authParam: { type: "text"; value: string } | { type: "null" } =
            meta.authentication
              ? { type: "text", value: meta.authentication }
              : { type: "null" };
          await tx.executeRaw(
            `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
            [...dbParams(meta.vaultId, entry.name, meta.salt, meta.iv, meta.createdAt), authParam],
          );

          for (const s of secretsResults) {
            await tx.executeRaw(
              `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
              dbParams(s.id, vaultId, s.ciphertext, s.nonce, s.createdAt, s.updatedAt),
            );
          }

          for (const f of foldersResults) {
            await tx.executeRaw(
              `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
              dbParams(f.id, vaultId, f.ciphertext, f.nonce, f.createdAt, f.updatedAt),
            );
          }

          if (meta.lockSettings) {
            // lockSettings is encrypted JSON string { ciphertext, nonce }
            const parsed = JSON.parse(meta.lockSettings);
            await tx.executeRaw(
              `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
              dbParams(
                vaultId,
                parsed.ciphertext,
                parsed.nonce,
                new Date().toISOString(),
              ),
            );
          }

          await tx.commit();
        } catch (e) {
          await tx.rollback();
          throw e;
        }

        // Delete old per-vault DB
        await indexedDB.deleteDatabase(oldDbName);
        console.log(`Migrated vault ${vaultId} (${entry.name})`);
      } finally {
        oldClient.close();
        oldDb.close();
      }
    } catch (e) {
      // Preserve old DB on error — user can recover from backup
      console.error(`Migration failed for vault ${vaultId}:`, e);
      throw e;
    }
  }

  // 4. Delete registry IndexedDB
  await indexedDB.deleteDatabase(REGISTRY_DB_NAME);
  console.log("Migration complete: registry IndexedDB deleted");
}

/**
 * Read vault identities from the old registry IndexedDB.
 */
async function readRegistryIndexedDB(): Promise<
  { id: string; name: string; status: "pending" | "ready"; migrated: boolean }[]
> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onupgradeneeded = () => {
      // Don't create store - we're only reading
    };
    request.onsuccess = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("vaults")) {
        db.close();
        resolve([]);
        return;
      }
      const tx = db.transaction("vaults", "readonly");
      const store = tx.objectStore("vaults");
      const getAllRequest = store.getAll();
      getAllRequest.onsuccess = () => {
        const entries: { id: string; name: string; status: "pending" | "ready"; migrated: boolean }[] = getAllRequest.result;
        db.close();
        resolve(entries);
      };
      getAllRequest.onerror = () => {
        db.close();
        reject(getAllRequest.error);
      };
    };
    request.onerror = () => {
      // Registry DB doesn't exist - no migration needed
      resolve([]);
    };
    request.onblocked = () => {
      reject(new Error("Registry upgrade blocked"));
    };
  });
}

/**
 * Check if migration is needed (has legacy data but no shared tables data).
 */
export async function needsMigration(): Promise<boolean> {
  const sharedDb = SharedDb.getInstance();
  const client = await sharedDb.getClient();

  // Check if shared tables have data
  const existing = await client.execute(
    `SELECT id FROM vaults LIMIT 1`,
    () => 1,
  );
  if (existing.length > 0) return false; // Already migrated

  // Check if registry IndexedDB exists and has entries
  try {
    const entries = await readRegistryIndexedDB();
    return entries.length > 0;
  } catch {
    return false;
  }
}
