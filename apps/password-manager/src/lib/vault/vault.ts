import type { Client } from "@ofsdk/sql";
import {
  createEncryptedVault,
  unlockEncryptedVault,
  type VaultDatabase,
  type VaultHandle,
} from "./encrypted-vault";
import {
  getSharedClient,
  migrateToSharedTables,
  needsMigration,
  openSharedDatabase,
} from "./storage/migrate-to-shared";
import { dbParams, localEncryptedRows, parseVaultRow } from "./storage/schema";
import { createVaultRegistry, type VaultIdentity } from "./storage/VaultRegistry";

function localDatabase(client: Client): VaultDatabase {
  return {
    read: (sql, parse, params) => client.execute(sql, parse, dbParams(...params)),
    async write(sql, params) {
      await client.executeRaw(sql, dbParams(...params));
    },
    async writeBatch(statements) {
      const tx = await client.transaction();
      try {
        for (const { sql, params } of statements) await tx.executeRaw(sql, dbParams(...params));
        await tx.commit();
      } catch (error) {
        await tx.rollback();
        throw error;
      }
    },
    parseMeta: parseVaultRow,
    encryptedRows: localEncryptedRows,
    async verifyExists(id) {
      await createVaultRegistry(client).findVault(id);
    },
    async withItemLock(id, action) {
      return navigator.locks.request(`password-manager-items-${id}`, action);
    },
    async close() {
      // The local client lifecycle is managed by the caller.
    },
  };
}

export async function createVault(password: string, name: string): Promise<VaultHandle> {
  return createEncryptedVault(localDatabase(await getSharedClient()), password, name);
}

export async function unlockVault(password: string, id: string): Promise<VaultHandle> {
  if (await needsMigration()) await migrateToSharedTables();
  const vault = await unlockEncryptedVault(localDatabase(await getSharedClient()), password, id);
  if (!vault) throw new Error("Vault not found");
  return vault;
}

export async function listVaults(): Promise<
  { id: string; name: string; status: "ready"; migrated: boolean }[]
> {
  const database = await openSharedDatabase();
  try {
    // Merely opening the selector must not create independent catalog history.
    if (await database.initialRestoreIsEligible()) return [];
  } finally {
    database.close();
  }
  const client = await getSharedClient();
  try {
    return await createVaultRegistry(client).listVaultIdentities();
  } finally {
    client.close();
  }
}

export type { VaultHandle, VaultIdentity };
