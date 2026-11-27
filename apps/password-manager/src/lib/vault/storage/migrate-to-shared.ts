import { type Client, type Row, Database as SqlDatabase } from "@ofsdk/sql";
import { dbParams, MIGRATIONS, parseOptionalText, parseText, parseVaultRow } from "./schema";
import type { VaultIdentity } from "./VaultRegistry";

const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
const LEGACY_DB_PREFIX = "password-manager-local-";

interface LegacyVaultIdentity {
  id: string;
  name: string;
  status: "pending" | "ready";
  migrated: boolean;
  copiedToShared?: boolean;
}

interface LegacyVaultMeta {
  vaultId: string;
  salt: string;
  iv: string;
  createdAt: string;
  authentication: string | null;
  lockSettings: string | null;
}

const SHARED_DB_NAME = "password-manager-v1";

export function openSharedDatabase(): Promise<SqlDatabase> {
  return SqlDatabase.open(SHARED_DB_NAME);
}

export async function getSharedClient(): Promise<Client> {
  return navigator.locks.request(`${SHARED_DB_NAME}:migrations`, async () => {
    const db = await openSharedDatabase();
    const client = db.client();
    try {
      await client.migrate(MIGRATIONS);
      return client;
    } catch (error) {
      client.close();
      throw error;
    } finally {
      db.close();
    }
  });
}

async function pendingLocals() {
  if (!(await indexedDB.databases()).some(({ name }) => name === REGISTRY_DB_NAME)) return [];
  return (await readRegistryIndexedDB()).filter((entry) => !entry.copiedToShared);
}

export async function listLegacyLocalVaults(): Promise<VaultIdentity[]> {
  return (await pendingLocals()).map(({ id, name, migrated }) => ({
    id,
    name,
    migrated,
    status: "ready",
  }));
}

export async function hasLegacyLocalVaults(): Promise<boolean> {
  return (await pendingLocals()).length > 0;
}

export async function migrateLocalVault(id: string): Promise<void> {
  await navigator.locks.request(`${SHARED_DB_NAME}:legacy-local`, async () => {
    const entry = (await pendingLocals()).find((entry) => entry.id === id);
    if (!entry) return;
    const client = await getSharedClient();
    try {
      await migrateLegacyVault(client, entry);
      // The registry records only completion of the copy; it never authorizes removal of the source.
      await markCopied(entry);
    } finally {
      client.close();
    }
  });
}

async function migrateLegacyVault(
  sharedClient: Client,
  identity: LegacyVaultIdentity
): Promise<void> {
  const vaultId = identity.id;
  const legacyDbName = `${LEGACY_DB_PREFIX}${vaultId}`;
  if (!(await indexedDB.databases()).some(({ name }) => name === `ofdb:sql:${legacyDbName}`))
    throw new Error(`Legacy local source not found: ${vaultId}. Do not clear browser data.`);
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
    if (!meta || meta.vaultId !== vaultId)
      throw new Error(`Legacy vault metadata not found: ${vaultId}`);
    const [existing] = await sharedClient.execute(
      `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
      parseVaultRow,
      dbParams(vaultId)
    );
    if (existing) {
      if (
        existing.salt !== meta.salt ||
        existing.iv !== meta.iv ||
        existing.createdAt !== meta.createdAt ||
        existing.authentication !== meta.authentication
      )
        throw new Error(
          `The local vault ID conflicts with the older copy: ${vaultId}. Both copies were kept.`
        );
      return;
    }

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
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error("Legacy registry read blocked"));
  });
}

async function markCopied(entry: LegacyVaultIdentity): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error("Legacy registry write blocked"));
    request.onsuccess = () => {
      const db = request.result;
      try {
        const transaction = db.transaction("vaults", "readwrite");
        transaction.objectStore("vaults").put({ ...entry, copiedToShared: true });
        transaction.oncomplete = () => {
          db.close();
          resolve();
        };
        transaction.onabort = () => {
          db.close();
          reject(transaction.error);
        };
      } catch (error) {
        db.close();
        reject(error);
      }
    };
  });
}
