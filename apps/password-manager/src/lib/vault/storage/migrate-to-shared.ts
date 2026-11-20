import { Database as SqlDatabase, type Client, type Row } from "@ofsdk/sql";
import type { DatabaseHandle } from "@ofsdk/sync-sdk";
import { unlockSyncVault, type SyncVaultHandle } from "../sync-vault";
import {
  parseSecretRow,
  parseFolderRow,
  parseVaultRow,
  parseText,
  parseOptionalText,
  MIGRATIONS,
  dbParams,
  dbParam,
} from "./schema";

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
  identity: LegacyVaultIdentity,
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
      }),
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
      }),
    );
    const folders = await legacyClient.execute(
      `SELECT id, ciphertext, nonce, created_at, updated_at FROM folders`,
      (row: Row) => ({
        id: parseText(row, "id"),
        ciphertext: parseText(row, "ciphertext"),
        nonce: parseText(row, "nonce"),
        createdAt: parseText(row, "created_at"),
        updatedAt: parseText(row, "updated_at"),
      }),
    );

    const tx = await sharedClient.transaction();
    try {
      const authentication = meta.authentication
        ? { type: "text" as const, value: meta.authentication }
        : { type: "null" as const };
      await tx.executeRaw(
        `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
        [...dbParams(meta.vaultId, identity.name, meta.salt, meta.iv, meta.createdAt), authentication],
      );
      for (const secret of secrets) {
        await tx.executeRaw(
          `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
          dbParams(secret.id, vaultId, secret.ciphertext, secret.nonce, secret.createdAt, secret.updatedAt),
        );
      }
      for (const folder of folders) {
        await tx.executeRaw(
          `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
          dbParams(folder.id, vaultId, folder.ciphertext, folder.nonce, folder.createdAt, folder.updatedAt),
        );
      }
      if (meta.lockSettings) {
        const settings = JSON.parse(meta.lockSettings);
        await tx.executeRaw(
          `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
          dbParams(vaultId, settings.ciphertext, settings.nonce, new Date().toISOString()),
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

/**
 * NEW: Migrate an existing local vault to a synced vault.
 * This runs when an authenticated user unlocks a vault that was created locally.
 *
 * Steps:
 * 1. Create/open sync resource for this vault
 * 2. Open local WASM DB with the sync resourceId
 * 3. Run initial sync (uploads all local data to Storage Service)
 * 4. Delete old per-vault DBs after successful sync
 */
export async function migrateLocalVaultToSync(
  vaultId: string,
  password: string,
  syncClient: { createDatabase: (name: string) => Promise<DatabaseHandle>; openDatabase: (id: string) => Promise<DatabaseHandle> }
): Promise<{ syncHandle: DatabaseHandle; syncDb: SyncVaultHandle }> {
  // First, get the local vault data
  const localClient = await getSharedClient();

  const vaultResults = await localClient.execute(
    `SELECT id, name, salt, iv, created_at, authentication FROM vaults WHERE id = ?`,
    (row: Row) => parseVaultRow(row),
    [dbParam(vaultId)],
  );

  if (!vaultResults[0]) {
    localClient.close();
    throw new Error("Vault not found");
  }

  const vault = vaultResults[0];

  // Verify password
  const cryptoModule = await import("../storage/crypto");
  const { fromBase64, deriveKey, decrypt, verifyAuthentication } = cryptoModule;
  const salt = fromBase64(vault.salt);
  const key = await deriveKey(password, salt);

  if (vault.authentication) {
    await verifyAuthentication(key, { id: vault.id, salt: vault.salt, iv: vault.iv, createdAt: vault.createdAt }, vault.authentication);
  } else {
    const secretRows = await localClient.execute(
      `SELECT id, ciphertext, nonce FROM secrets WHERE vault_id = ?`,
      (row: Row) => ({
        id: (row.id as { type: "uuid"; value: string }).value,
        ciphertext: (row.ciphertext as { type: "text"; value: string }).value,
        nonce: (row.nonce as { type: "text"; value: string }).value,
      }),
      [dbParam(vaultId)],
    );
    if (secretRows.length === 0) {
      localClient.close();
      throw new Error("original password cannot be verified");
    }
    for (const s of secretRows) {
      try {
        await decrypt(key, s.ciphertext, s.nonce);
      } catch {
        localClient.close();
        throw new Error("Wrong password or damaged legacy records");
      }
    }
  }

  // Read all vault data
  const secretResults = await localClient.execute(
    `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
    (row: Row) => parseSecretRow(row),
    [dbParam(vaultId)],
  );

  const folderResults = await localClient.execute(
    `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
    (row: Row) => parseFolderRow(row),
    [dbParam(vaultId)],
  );

  const lockResults = await localClient.execute(
    `SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
    (row: Row): { vault_id: string; ciphertext: string; nonce: string; updated_at: string } => parseLockSettingsRow(row),
    [dbParam(vaultId)],
  );

  localClient.close();

  // Create sync resource for this vault
  const syncHandle = await syncClient.createDatabase(`vault-${vaultId}`);
  const syncDb = await SqlDatabase.open(syncHandle.id);

  // Migrate all data to the sync database
  const tx = await syncDb.client().transaction();
  try {
    // Insert vault metadata
    await tx.executeRaw(
      `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
      dbParams(vaultId, vault.name, vault.salt, vault.iv, vault.createdAt, vault.authentication),
    );

    // Insert secrets
    for (const secret of secretResults) {
      await tx.executeRaw(
        `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
        dbParams(secret.id, vaultId, secret.ciphertext, secret.nonce, secret.createdAt, secret.updatedAt),
      );
    }

    // Insert folders
    for (const folder of folderResults) {
      await tx.executeRaw(
        `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
        dbParams(folder.id, vaultId, folder.ciphertext, folder.nonce, folder.createdAt, folder.updatedAt),
      );
    }

    // Insert lock_settings
    if (lockResults[0]) {
      await tx.executeRaw(
        `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
        dbParams(lockResults[0].vault_id, lockResults[0].ciphertext, lockResults[0].nonce, lockResults[0].updated_at),
      );
    }

    await tx.commit();
  } catch (e) {
    await tx.rollback();
    throw e;
  }

  // Run initial sync to upload all data
  await syncHandle.sync({ direction: "push" });

  // TODO: Delete old per-vault DBs after successful sync
  // await SqlDatabase.delete(`password-manager-${vaultId}`);

  const syncedVault = await unlockSyncVault(password, vaultId, syncHandle);
  if (!syncedVault) throw new Error("Migrated vault not found");
  return { syncHandle, syncDb: syncedVault };
}

function parseLockSettingsRow(row: Row) {
  return {
    vault_id: (row.vault_id as { type: "uuid"; value: string }).value,
    ciphertext: (row.ciphertext as { type: "text"; value: string }).value,
    nonce: (row.nonce as { type: "text"; value: string }).value,
    updated_at: (row.updated_at as { type: "text"; value: string }).value,
  };
}
