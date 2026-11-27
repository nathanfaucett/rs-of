import { Database } from "@ofsdk/sql";
import { unlockEncryptedVault } from "./encrypted-vault";
import { vaultHistoryRecords } from "./history-records";
import { MIGRATIONS, parseText, syncedEncryptedRows } from "./storage/schema";
import { createVaultRegistry } from "./storage/VaultRegistry";
import { localDatabase } from "./vault";

export const legacyCreationKey = "password-manager-main";
export const resourceIdPattern = /^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i;

// Released browser replicas used the Resource UUID in this on-disk namespace.
export async function openLegacyReplica(id: string): Promise<Database | undefined> {
  if (!resourceIdPattern.test(id)) throw new Error("Enter the exact Storage Resource ID.");
  if ((await indexedDB.databases()).some(({ name }) => name === `ofdb:sql:${id}`)) {
    return Database.open(id);
  }
  return undefined;
}

async function normalize(source: Database) {
  if (await source.initialRestoreIsEligible()) return;
  let stage: Database;
  let needsLedger = false;
  try {
    stage = await source.stageHistory(source, vaultHistoryRecords);
  } catch {
    // The old app ran the same DDL directly, without the migration ledger.
    stage = await source.stageHistory(
      source,
      vaultHistoryRecords.filter(({ table }) => table !== "_ofdb_migrations")
    );
    needsLedger = true;
  }
  const client = stage.client();
  try {
    const applied = (await client.migrate(MIGRATIONS)).appliedVersions.length;
    if (applied !== (needsLedger ? MIGRATIONS.length : 0))
      throw new Error("The older database does not have the expected vault schema.");
    return stage;
  } catch (error) {
    stage.close();
    throw error;
  } finally {
    client.close();
  }
}

export async function stageLegacyUpgrade(local: Database, remote: Database, browser?: Database) {
  const server = await normalize(remote);
  let retained: Database | undefined;
  let combined: Database | undefined;
  try {
    retained = browser && (await normalize(browser));
    combined =
      server && retained
        ? await server.stageHistory(retained, vaultHistoryRecords)
        : (server ?? retained);
    if (!combined) throw new Error("The older Resource has no Password Manager history.");
    const client = combined.client();
    let vaults: string[];
    try {
      vaults = await client.execute("SELECT id FROM vaults WHERE status = 'ready'", (row) =>
        parseText(row, "id")
      );
    } finally {
      client.close();
    }
    // Even a database with no visible vaults can contain retained deletion history.
    const stage = (await local.initialRestoreIsEligible())
      ? await combined.stageHistory(combined, vaultHistoryRecords)
      : await local.stageHistory(combined, vaultHistoryRecords);
    return { stage, vaults };
  } finally {
    if (combined !== server && combined !== retained) combined?.close();
    retained?.close();
    server?.close();
  }
}

export async function listLegacyRecovery(id: string) {
  const database = await openLegacyReplica(id);
  if (!database) throw new Error("This browser has no retained copy of that Resource.");
  const client = database.client();
  try {
    return await createVaultRegistry(client).listVaultIdentities();
  } finally {
    client.close();
    database.close();
  }
}

export async function unlockLegacyRecovery(id: string, vaultId: string, password: string) {
  const database = await openLegacyReplica(id);
  if (!database) throw new Error("This browser has no retained copy of that Resource.");
  const adapter = localDatabase(database.client(), true);
  database.close();
  const readOnly = async () => {
    throw new Error(
      "The retained recovery copy is read-only. Export an encrypted backup to keep working."
    );
  };
  try {
    const vault = await unlockEncryptedVault(
      { ...adapter, encryptedRows: syncedEncryptedRows, write: readOnly, writeBatch: readOnly },
      password,
      vaultId
    );
    if (!vault) throw new Error("Vault not found in the retained copy.");
    return vault;
  } catch (error) {
    await adapter.close();
    throw error;
  }
}
