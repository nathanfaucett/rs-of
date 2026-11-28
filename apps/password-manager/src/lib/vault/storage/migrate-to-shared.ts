import { type Client, type Row, Database as SqlDatabase, type Transaction } from "@ofsdk/sql";
import * as v from "valibot";
import { unlockEncryptedVault, type VaultDatabase } from "../encrypted-vault";
import { vaultMigrations } from "../migrations";
import { openLegacyReplicaFence } from "./retire-legacy-replica";
import {
  dbParams,
  MIGRATIONS,
  parseInteger,
  parseOptionalText,
  parseText,
  parseVaultRow,
  sharedEncryptedRows,
  VaultRowSchema,
} from "./schema";
import type { VaultIdentity } from "./VaultRegistry";
import { requireVaultLayout, validateVaultRows } from "./validate-vault-database";

const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
const LEGACY_DB_PREFIX = "password-manager-local-";

interface LegacyVaultIdentity {
  id: string;
  name: string;
  status: "pending" | "ready";
  migrated: boolean;
  verifiedInShared?: boolean;
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
  // The earlier copiedToShared marker did not verify the contents or retire writers.
  return (await readRegistryIndexedDB()).filter((entry) => !entry.verifiedInShared);
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

const fields = "id, ciphertext, nonce, created_at, updated_at";
const vaultFields = "id, name, salt, iv, created_at, authentication, status, migrated";
const envelope = v.strictObject({ ciphertext: v.string(), nonce: v.string() });
const parseEnvelope = (row: Row) => ({
  ciphertext: parseText(row, "ciphertext"),
  nonce: parseText(row, "nonce"),
});
const parseItem = (row: Row) => ({
  id: parseText(row, "id"),
  ...parseEnvelope(row),
  createdAt: parseText(row, "created_at"),
  updatedAt: parseText(row, "updated_at"),
});

async function readSource({ id, name }: LegacyVaultIdentity) {
  const logicalName = `${LEGACY_DB_PREFIX}${id}`;
  const fence = await openLegacyReplicaFence(logicalName);
  if (!fence) throw new Error(`Legacy local source not found: ${id}. Do not clear browser data.`);
  try {
    return { fence, copy: await readLocalCopy(logicalName, id, name) };
  } catch (error) {
    fence.close();
    throw error;
  }
}

async function readLocalCopy(logicalName: string, id: string, name: string) {
  const db = await SqlDatabase.open(logicalName);
  const client = db.client();
  try {
    const tx = await client.transaction();
    try {
      await requireVaultLayout(tx, vaultMigrations);
      const [row, extra] = await tx.execute(
        `SELECT id, vault_id, salt, iv, created_at, authentication, lock_settings, item_format, transition_backup FROM vault_meta`,
        (row) => row
      );
      if (!row || extra || parseInteger(row, "id") !== 1 || parseText(row, "vault_id") !== id)
        throw new Error(`Invalid legacy vault metadata: ${id}`);
      if (
        parseInteger(row, "item_format") !== 1 ||
        parseOptionalText(row, "transition_backup") !== null
      )
        throw new Error("The older vault has an unsupported item transition. Keep browser data.");
      const vault = v.parse(VaultRowSchema, {
        id,
        name,
        salt: parseText(row, "salt"),
        iv: parseText(row, "iv"),
        createdAt: parseText(row, "created_at"),
        authentication: parseOptionalText(row, "authentication"),
        status: "ready",
        migrated: true,
      });
      const saved = parseOptionalText(row, "lock_settings");
      return {
        vault,
        secrets: await tx.execute(`SELECT ${fields} FROM secrets ORDER BY id`, parseItem),
        folders: await tx.execute(`SELECT ${fields} FROM folders ORDER BY id`, parseItem),
        lock: saved === null ? [] : [v.parse(envelope, JSON.parse(saved))],
      };
    } finally {
      tx.close();
    }
  } finally {
    client.close();
    db.close();
  }
}

type Copy = Awaited<ReturnType<typeof readLocalCopy>>;

async function insertCopy(tx: Transaction, { vault, secrets, folders, lock }: Copy) {
  await tx.executeRaw(`INSERT INTO vaults (${vaultFields}) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`, [
    ...dbParams(vault.id, vault.name, vault.salt, vault.iv, vault.createdAt),
    vault.authentication == null ? { type: "null" } : { type: "text", value: vault.authentication },
  ]);
  for (const [table, items] of [
    ["secrets", secrets],
    ["folders", folders],
  ] as const) {
    for (const item of items) {
      await tx.executeRaw(
        `INSERT INTO ${table} (vault_id, ${fields}) VALUES (?, ?, ?, ?, ?, ?)`,
        dbParams(vault.id, item.id, item.ciphertext, item.nonce, item.createdAt, item.updatedAt)
      );
    }
  }
  for (const settings of lock) {
    await tx.executeRaw(
      `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
      dbParams(vault.id, settings.ciphertext, settings.nonce, new Date().toISOString())
    );
  }
}

async function checkCopy(tx: Transaction, original: Copy) {
  const params = dbParams(original.vault.id);
  const [vault, extra] = await tx.execute(
    `SELECT ${vaultFields} FROM vaults WHERE id = ?`,
    (row) => {
      if (parseText(row, "status") !== "ready") throw new Error("Invalid copied vault status.");
      return parseVaultRow(row);
    },
    params
  );
  const copy = {
    vault,
    secrets: await tx.execute(
      `SELECT ${fields} FROM secrets WHERE vault_id = ? ORDER BY id`,
      parseItem,
      params
    ),
    folders: await tx.execute(
      `SELECT ${fields} FROM folders WHERE vault_id = ? ORDER BY id`,
      parseItem,
      params
    ),
    lock: await tx.execute(
      `SELECT ciphertext, nonce FROM lock_settings WHERE vault_id = ?`,
      parseEnvelope,
      params
    ),
  };
  if (extra || JSON.stringify(copy) !== JSON.stringify(original))
    throw new Error(
      "The shared vault does not match the older source. Both copies were kept. Open the recovery copy to export a backup."
    );
}

function readOnlyDatabase(reader: Pick<Client, "execute">, close: () => void): VaultDatabase {
  const readOnly = async () => {
    throw new Error("The retained recovery copy is read-only.");
  };
  return {
    read: (sql, parse, params) => reader.execute(sql, parse, dbParams(...params)),
    write: readOnly,
    writeBatch: readOnly,
    parseMeta: parseVaultRow,
    encryptedRows: sharedEncryptedRows,
    async close() {
      close();
    },
  };
}

export async function migrateLocalVault(id: string, password: string): Promise<void> {
  await navigator.locks.request(`${SHARED_DB_NAME}:legacy-local`, async () => {
    const entry = (await pendingLocals()).find((entry) => entry.id === id);
    if (!entry) return;
    const { fence, copy: original } = await readSource(entry);
    try {
      const verification = await openRecoveryCopy(original, password, true);
      await verification.close();
      const db = await openSharedDatabase();
      const client = db.client();
      try {
        if (await db.initialRestoreIsEligible()) await client.migrate(MIGRATIONS);
        const tx = await client.transaction();
        try {
          await requireVaultLayout(tx, MIGRATIONS);
          const exists = await tx.execute(
            "SELECT id FROM vaults WHERE id = ?",
            (row) => row,
            dbParams(id)
          );
          if (!exists.length) await insertCopy(tx, original);
          await checkCopy(tx, original);
          await fence.retire();
          await tx.commit();
        } finally {
          tx.close();
        }
        // Compare the persisted ciphertext to the copy that just decrypted successfully.
        const saved = await client.transaction();
        try {
          await checkCopy(saved, original);
        } finally {
          saved.close();
        }
        await markVerified(entry);
      } finally {
        client.close();
        db.close();
      }
    } finally {
      fence.close();
    }
  });
}

export async function unlockLocalRecovery(id: string, password: string) {
  const entry = (await pendingLocals()).find((entry) => entry.id === id);
  if (!entry) throw new Error("This older vault is no longer pending verification.");
  const { fence, copy } = await readSource(entry);
  fence.close();
  return openRecoveryCopy(copy, password);
}

async function openRecoveryCopy(source: Copy, password: string, verify = false) {
  const db = await SqlDatabase.inMemory();
  const client = db.client();
  const reader = readOnlyDatabase(client, () => client.close());
  try {
    await client.migrate(MIGRATIONS);
    const tx = await client.transaction();
    try {
      await insertCopy(tx, source);
      await tx.commit();
    } finally {
      tx.close();
    }
    const vault = await unlockEncryptedVault(reader, password, source.vault.id);
    if (!vault) throw new Error("Older vault not found.");
    if (verify) {
      await vault.loadSecrets();
      await vault.loadFolders();
      await vault.loadLockSettings();
      await validateVaultRows(db, true);
    }
    return vault;
  } catch (error) {
    await reader.close();
    throw error;
  } finally {
    db.close();
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

async function markVerified(entry: LegacyVaultIdentity): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error("Legacy registry write blocked"));
    request.onsuccess = () => {
      const db = request.result;
      try {
        const transaction = db.transaction("vaults", "readwrite");
        const store = transaction.objectStore("vaults");
        const request = store.get(entry.id);
        request.onsuccess = () => {
          const current = request.result as LegacyVaultIdentity | undefined;
          if (!current || current.name !== entry.name) transaction.abort();
          else store.put({ ...current, verifiedInShared: true });
        };
        transaction.oncomplete = () => {
          db.close();
          resolve();
        };
        transaction.onabort = () => {
          db.close();
          reject(
            transaction.error ??
              new Error("Could not save legacy verification. Unlock again to retry.")
          );
        };
      } catch (error) {
        db.close();
        reject(error);
      }
    };
  });
}
