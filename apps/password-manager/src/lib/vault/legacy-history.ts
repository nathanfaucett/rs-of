import { Database } from "@ofsdk/sql";
import { unlockEncryptedVault } from "./encrypted-vault";
import { vaultHistoryRecords } from "./history-records";
import { openLegacyReplicaFence, retiredRevision } from "./storage/retire-legacy-replica";
import { MIGRATIONS, syncedEncryptedRows } from "./storage/schema";
import { createVaultRegistry } from "./storage/VaultRegistry";
import {
  hasVaultMigrationLedger,
  ledgerlessVaultRecords,
  validateVaultDatabase,
  validateVaultRows,
} from "./storage/validate-vault-database";
import { localDatabase } from "./vault";

export const legacyCreationKey = "password-manager-main";
export const resourceIdPattern = /^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i;

// Released browser replicas used the Resource UUID in this on-disk namespace.
export async function captureLegacyReplica(id: string) {
  if (!resourceIdPattern.test(id)) throw new Error("Enter the exact Storage Resource ID.");
  const fence = await openLegacyReplicaFence(id);
  if (!fence) return;
  let database: Database | undefined;
  try {
    const replica = await Database.open(id);
    database = replica;
    if (fence.revision === retiredRevision && (await replica.initialRestoreIsEligible())) {
      // An earlier cutover could reserve an absent replica without writing vault history.
      replica.close();
      fence.close();
      return;
    }
    return {
      database: replica,
      revision: fence.revision,
      retire: fence.retire,
      close: () => {
        replica.close();
        fence.close();
      },
    };
  } catch (error) {
    database?.close();
    fence.close();
    throw error;
  }
}

async function normalize(source: Database, requireVault: boolean) {
  const hasLedger = await hasVaultMigrationLedger(source);
  const records = hasLedger ? vaultHistoryRecords : ledgerlessVaultRecords;
  const stage = await source.stageHistory(source, records);
  const client = stage.client();
  try {
    await validateVaultDatabase(stage, hasLedger, requireVault);
    // Released synced replicas ran this same DDL without the ledger. Add only that known plan.
    if (
      !hasLedger &&
      (await client.migrate(MIGRATIONS)).appliedVersions.length !== MIGRATIONS.length
    )
      throw new Error("The older database does not have the expected vault migration history.");
    return stage;
  } catch (error) {
    stage.close();
    throw error;
  } finally {
    client.close();
  }
}

export async function stageLegacyUpgrade(
  local: Database,
  remote: Database,
  exactId: boolean,
  browser?: Database
) {
  // A catalog ID only proves namespace membership, not that this is a vault Resource.
  const server = await normalize(remote, exactId);
  let retained: Database | undefined;
  let combined: Database | undefined;
  try {
    retained = browser && (await normalize(browser, false));
    combined = retained ? await server.stageHistory(retained, vaultHistoryRecords) : server;
    const vaults = await validateVaultRows(combined, false);
    const stage = (await local.initialRestoreIsEligible())
      ? await combined.stageHistory(combined, vaultHistoryRecords)
      : await local.stageHistory(combined, vaultHistoryRecords);
    return { stage, vaults };
  } finally {
    if (combined !== server) combined?.close();
    retained?.close();
    server.close();
  }
}

export async function listLegacyRecovery(id: string) {
  const replica = await captureLegacyReplica(id);
  if (!replica) throw new Error("This browser has no retained copy of that Resource.");
  const client = replica.database.client();
  try {
    return await createVaultRegistry(client).listVaultIdentities();
  } finally {
    client.close();
    replica.close();
  }
}

export async function unlockLegacyRecovery(id: string, vaultId: string, password: string) {
  const replica = await captureLegacyReplica(id);
  if (!replica) throw new Error("This browser has no retained copy of that Resource.");
  const adapter = localDatabase(replica.database.client(), true);
  replica.close();
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
