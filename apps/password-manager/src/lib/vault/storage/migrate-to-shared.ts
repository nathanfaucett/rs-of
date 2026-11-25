import { type Client, type Row, Database as SqlDatabase } from "@ofsdk/sql";
import { dbParams, MIGRATIONS, parseOptionalText, parseText } from "./schema";

const SHARED_DB_NAME = "password-manager-v1";
const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
const LEGACY_DB_PREFIX = "password-manager-local-";

interface LegacyVaultIdentity {
  id: string;
  name: string;
  status: "pending" | "ready";
  migrated: boolean;
}

interface LegacyVaultMeta {
  vaultId: string;
  salt: string;
  iv: string;
  createdAt: string;
  authentication: string | null;
  lockSettings: string | null;
}

/**
 * Get a client for the shared database (non-sync mode).
 * This replaces SharedDb.getInstance().getClient() for migration purposes.
 */
export async function getSharedClient(): Promise<Client> {
  return navigator.locks.request(`${SHARED_DB_NAME}:migrations`, async () => {
    const db = await SqlDatabase.open(SHARED_DB_NAME);
    const client = db.client();
    try {
      await client.migrate(MIGRATIONS);
      return client;
    } catch (error) {
      client.close();
      db.close();
      throw error;
    }
  });
}

/** One-time migration from per-vault tables and the registry to shared tables. */
export async function migrateToSharedTables(): Promise<void> {
  const client = await getSharedClient();
  try {
    const existing = await client.execute(`SELECT id FROM vaults LIMIT 1`, () => 1);
    if (existing.length > 0) return;

    const entries = await readRegistryIndexedDB();
    for (const entry of entries) {
      await migrateLegacyVault(client, entry);
    }
    await deleteIndexedDatabase(REGISTRY_DB_NAME);
  } finally {
    client.close();
  }
}

/** Check whether registered legacy vaults still need migration. */
export async function needsMigration(): Promise<boolean> {
  const client = await getSharedClient();
  try {
    const existing = await client.execute(`SELECT id FROM vaults LIMIT 1`, () => 1);
    if (existing.length > 0) return false;
    const entries = await readRegistryIndexedDB();
    return entries.length > 0;
  } catch {
    return false;
  } finally {
    client.close();
  }
}

async function migrateLegacyVault(
  sharedClient: Client,
  identity: LegacyVaultIdentity
): Promise<void> {
  const vaultId = identity.id;
  const legacyDbName = `${LEGACY_DB_PREFIX}${vaultId}`;
  const legacyDb = await SqlDatabase.open(legacyDbName);
  const legacyClient = legacyDb.client();

  try {
    const metaResults = await legacyClient.execute(
      `SELECT vault_id, salt, iv, created_at, authentication, lock_settings FROM vault_meta WHERE id = 1`,
      (row: Row): LegacyVaultMeta => ({
        vaultId: parseText(row, "vault_id"),
        salt: parseText(row, "salt"),
        iv: parseText(row, "iv"),
        createdAt: parseText(row, "created_at"),
        authentication: parseOptionalText(row, "authentication"),
        lockSettings: parseOptionalText(row, "lock_settings"),
      })
    );
    const meta = metaResults[0];
    if (!meta) throw new Error(`Legacy vault metadata not found: ${vaultId}`);

    const secrets = await legacyClient.execute(
      `SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets`,
      (row: Row) => ({
        id: parseText(row, "id"),
        ciphertext: parseText(row, "ciphertext"),
        nonce: parseText(row, "nonce"),
        createdAt: parseText(row, "created_at"),
        updatedAt: parseText(row, "updated_at"),
      })
    );
    const folders = await legacyClient.execute(
      `SELECT id, ciphertext, nonce, created_at, updated_at FROM folders`,
      (row: Row) => ({
        id: parseText(row, "id"),
        ciphertext: parseText(row, "ciphertext"),
        nonce: parseText(row, "nonce"),
        createdAt: parseText(row, "created_at"),
        updatedAt: parseText(row, "updated_at"),
      })
    );

    const tx = await sharedClient.transaction();
    try {
      const authentication = meta.authentication
        ? { type: "text" as const, value: meta.authentication }
        : { type: "null" as const };
      await tx.executeRaw(
        `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
        [
          ...dbParams(meta.vaultId, identity.name, meta.salt, meta.iv, meta.createdAt),
          authentication,
        ]
      );
      for (const secret of secrets) {
        await tx.executeRaw(
          `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
          dbParams(
            secret.id,
            vaultId,
            secret.ciphertext,
            secret.nonce,
            secret.createdAt,
            secret.updatedAt
          )
        );
      }
      for (const folder of folders) {
        await tx.executeRaw(
          `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
          dbParams(
            folder.id,
            vaultId,
            folder.ciphertext,
            folder.nonce,
            folder.createdAt,
            folder.updatedAt
          )
        );
      }
      if (meta.lockSettings) {
        const settings = JSON.parse(meta.lockSettings);
        await tx.executeRaw(
          `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
          dbParams(vaultId, settings.ciphertext, settings.nonce, new Date().toISOString())
        );
      }
      await tx.commit();
    } catch (error) {
      await tx.rollback();
      throw error;
    }
  } finally {
    legacyClient.close();
    legacyDb.close();
  }

  await deleteIndexedDatabase(legacyDbName);
}

async function readRegistryIndexedDB(): Promise<LegacyVaultIdentity[]> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onupgradeneeded = () => {};
    request.onsuccess = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("vaults")) {
        db.close();
        resolve([]);
        return;
      }
      const getAllRequest = db.transaction("vaults", "readonly").objectStore("vaults").getAll();
      getAllRequest.onsuccess = () => {
        const entries = getAllRequest.result as LegacyVaultIdentity[];
        db.close();
        resolve(entries);
      };
      getAllRequest.onerror = () => {
        db.close();
        reject(getAllRequest.error);
      };
    };
    request.onerror = () => resolve([]);
    request.onblocked = () => reject(new Error("Legacy registry read blocked"));
  });
}

async function deleteIndexedDatabase(name: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.deleteDatabase(name);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error(`Database deletion blocked: ${name}`));
  });
}
