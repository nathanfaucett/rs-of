import { Database as SqlDatabase, Client, type Row, Transaction } from "@ofsdk/sql";
import * as v from "valibot";
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
} from "./schema";
import { type VaultIdentity } from "../vault";

const SHARED_DB_NAME = "password-manager-v1";

/**
 * Get a client for the shared database (non-sync mode).
 * This replaces SharedDb.getInstance().getClient() for migration purposes.
 */
export async function getSharedClient(): Promise<Client> {
  const db = await SqlDatabase.open(SHARED_DB_NAME);
  const client = db.client();
  await client.migrate(MIGRATIONS);
  return client;
}

/**
 * Migrate from legacy per-vault DBs + registry to shared tables.
 * This is the OLD migration (per-vault DBs → shared tables).
 * Now uses direct SqlDatabase instead of SharedDb singleton.
 */
export async function migrateToSharedTables(): Promise<void> {
  const client = await getSharedClient();

  // 1. Run migrations to create new tables (vaults, lock_settings, secrets, folders)
  await client.migrate(MIGRATIONS);

  // 2. Check if migration already done
  const existing = await client.execute(
    `SELECT id FROM vaults LIMIT 1`,
    () => 1,
  );
  if (existing.length > 0) {
    client.close();
    return; // Already migrated
  }

  // 3. Find all legacy vault databases
  const legacyDbs = await findLegacyVaultDatabases();
  if (legacyDbs.length === 0) {
    client.close();
    return; // No legacy vaults to migrate
  }

  // 4. Migrate each legacy vault
  for (const legacyDb of legacyDbs) {
    await migrateLegacyVault(client, legacyDb);
  }

  client.close();
}

/**
 * Check if migration from legacy per-vault DBs is needed.
 * Returns true if there are legacy vaults that haven't been migrated.
 */
export async function needsMigration(): Promise<boolean> {
  const client = await getSharedClient();

  // Check if shared tables have data
  const existing = await client.execute(
    `SELECT id FROM vaults LIMIT 1`,
    () => 1,
  );
  if (existing.length > 0) {
    client.close();
    return false; // Already migrated
  }

  // Check for legacy databases
  const legacyDbs = await findLegacyVaultDatabases();
  client.close();
  return legacyDbs.length > 0;
}

/**
 * Find all legacy vault databases in IndexedDB.
 * Legacy databases are named like "password-manager-{vaultId}".
 */
async function findLegacyVaultDatabases(): Promise<string[]> {
  // We need to check IndexedDB for databases matching the pattern
  // Since we can't easily list all IndexedDB databases from here,
  // we'll check the VaultRegistry which tracks known vaults
  const client = await getSharedClient();
  const registry = createVaultRegistry(client);
  const identities = await registry.listVaultIdentities();
  client.close();

  // Legacy vaults would have been tracked in the old registry
  // For now, we check if there are any vaults in the registry
  // but not in the shared tables
  return identities.map((v) => v.id);
}

/**
 * Migrate a single legacy vault to the shared tables.
 */
async function migrateLegacyVault(sharedClient: Client, vaultId: string): Promise<void> {
  // Open the legacy database
  const legacyDbName = `password-manager-${vaultId}`;
  let legacyDb: SqlDatabase | null = null;
  let legacyClient: Client | null = null;

  try {
    legacyDb = await SqlDatabase.open(legacyDbName);
    legacyClient = legacyDb.client();

    // Read vault metadata from legacy DB
    const vaultResults = await legacyClient.execute(
      `SELECT id, name, salt, iv, created_at, authentication FROM vaults WHERE id = ?`,
      (row: Row) => parseVaultRow(row),
      [dbParam(vaultId)],
    );

    if (!vaultResults[0]) return;

    const vault = vaultResults[0];

    // Read secrets from legacy DB
    const secretResults = await legacyClient.execute(
      `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
      (row: Row) => parseSecretRow(row),
      [dbParam(vaultId)],
    );

    // Read folders from legacy DB
    const folderResults = await legacyClient.execute(
      `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
      (row: Row) => parseFolderRow(row),
      [dbParam(vaultId)],
    );

    // Read lock_settings from legacy DB
    const lockResults = await legacyClient.execute(
      `SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
      (row: Row): { vault_id: string; ciphertext: string; nonce: string; updated_at: string } => parseLockSettingsRow(row),
      [dbParam(vaultId)],
    );

    // Insert into shared tables
    const tx = await sharedClient.transaction();
    try {
      // Insert vault metadata
      await tx.executeRaw(
        `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
        dbParams(vault.id, vault.name, vault.salt, vault.iv, vault.createdAt, vault.authentication),
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

      // Insert lock_settings if exists
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
  } finally {
    if (legacyClient) legacyClient.close();
    if (legacyDb) legacyDb.close();
  }
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
  syncClient: { createDatabase: (name: string) => Promise<any>; openDatabase: (id: string) => Promise<any> }
): Promise<any> {
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

  return { syncHandle, syncDb };
}

function parseLockSettingsRow(row: Row) {
  return {
    vault_id: (row.vault_id as { type: "uuid"; value: string }).value,
    ciphertext: (row.ciphertext as { type: "text"; value: string }).value,
    nonce: (row.nonce as { type: "text"; value: string }).value,
    updated_at: (row.updated_at as { type: "text"; value: string }).value,
  };
}
