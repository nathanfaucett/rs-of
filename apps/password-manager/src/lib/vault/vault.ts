import type { Client } from "@ofsdk/sql";
import {
  createEncryptedVault,
  unlockEncryptedVault,
  type VaultDatabase,
  type VaultHandle,
} from "./encrypted-vault";
import {
  getSharedClient,
  listLegacyLocalVaults,
  migrateLocalVault,
  openSharedDatabase,
} from "./storage/migrate-to-shared";
import { dbParams, parseVaultRow, sharedEncryptedRows } from "./storage/schema";
import { createVaultRegistry, type VaultIdentity } from "./storage/VaultRegistry";

export function localDatabase(client: Client, ownsClient = false): VaultDatabase {
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
    encryptedRows: sharedEncryptedRows,
    async verifyExists(id) {
      await createVaultRegistry(client).findVault(id);
    },
    async withItemLock(id, action) {
      return navigator.locks.request(`password-manager-items-${id}`, action);
    },
    async close() {
      if (ownsClient) client.close();
    },
  };
}

export async function createVault(password: string, name: string): Promise<VaultHandle> {
  const database = localDatabase(await getSharedClient(), true);
  try {
    return await createEncryptedVault(database, password, name);
  } catch (error) {
    await database.close();
    throw error;
  }
}

export async function unlockVault(password: string, id: string): Promise<VaultHandle> {
  await migrateLocalVault(id);
  const database = localDatabase(await getSharedClient(), true);
  try {
    const vault = await unlockEncryptedVault(database, password, id);
    if (!vault) throw new Error("Vault not found");
    return vault;
  } catch (error) {
    await database.close();
    throw error;
  }
}

export async function listVaults(): Promise<
  { id: string; name: string; status: "ready"; migrated: boolean }[]
> {
  const legacy = await listLegacyLocalVaults();
  const database = await openSharedDatabase();
  try {
    // Merely opening the selector must not create independent catalog history.
    if (await database.initialRestoreIsEligible()) return legacy;
  } finally {
    database.close();
  }
  const client = await getSharedClient();
  try {
    const shared = await createVaultRegistry(client).listVaultIdentities();
    const ids = new Set(shared.map(({ id }) => id));
    return [...shared, ...legacy.filter(({ id }) => !ids.has(id))];
  } finally {
    client.close();
  }
}

export type { VaultHandle, VaultIdentity };
