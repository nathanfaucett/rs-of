import { Client, type Row } from "@ofsdk/sql";
import * as v from "valibot";
import {
  parseVaultRow,
  type VaultRow,
  dbParams,
  dbParam,
} from "./schema";

export interface VaultIdentity {
  id: string;
  name: string;
  status: "ready";
  migrated: boolean;
}

/**
 * VaultRegistry replaces the IndexedDB registry (registry.ts).
 * All operations use the shared SQLite DB via typed execute().
 */
export class VaultRegistry {
  private client: Client;

  constructor(client: Client) {
    this.client = client;
  }

  /**
   * List all vault identities from the vaults table.
   * Used by the vault selector UI.
   */
  async listVaultIdentities(): Promise<VaultIdentity[]> {
    const results = await this.client.execute(
      `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults`,
      (row: Row) => {
        const vault = parseVaultRow(row);
        return {
          id: vault.id,
          name: vault.name,
          status: "ready" as const,
          migrated: vault.migrated,
        };
      },
    );
    return results;
  }

  /**
   * Find a specific vault by ID.
   * Throws if not found.
   */
  async findVault(id: string): Promise<VaultIdentity> {
    const results = await this.client.execute(
      `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
      (row: Row) => {
        const vault = parseVaultRow(row);
        return {
          id: vault.id,
          name: vault.name,
          status: "ready" as const,
          migrated: vault.migrated,
        };
      },
      [dbParam(id)],
    );
    if (!results[0]) throw new Error(`Vault ${id} not found`);
    return results[0];
  }

  /**
   * Reserve (create) a new vault in a single atomic transaction.
   * Inserts into vaults table with status='ready'.
   * No pending state - vault creation is atomic.
   */
  async reserveVault(
    name: string,
    salt: string,
    iv: string,
    authentication: string,
  ): Promise<VaultIdentity> {
    const id = crypto.randomUUID();
    const createdAt = new Date().toISOString();

    await this.client.executeRaw(
      `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
      dbParams(id, name, salt, iv, createdAt, authentication),
    );

    return { id, name, status: "ready", migrated: true };
  }

  /**
   * Delete a vault and all its data.
   * Manually deletes from all related tables since FK CASCADE is not used.
   */
  async deleteVault(id: string): Promise<void> {
    const tx = await this.client.transaction();
    try {
      await tx.executeRaw(`DELETE FROM secrets WHERE vault_id = ?`, [dbParam(id)]);
      await tx.executeRaw(`DELETE FROM folders WHERE vault_id = ?`, [dbParam(id)]);
      await tx.executeRaw(`DELETE FROM lock_settings WHERE vault_id = ?`, [dbParam(id)]);
      await tx.executeRaw(`DELETE FROM vaults WHERE id = ?`, [dbParam(id)]);
      await tx.commit();
    } catch (e) {
      await tx.rollback();
      throw e;
    }
  }

  /**
   * Get vault metadata for unlock (salt, iv, authentication).
   * Returns the full vault row for key derivation and verification.
   */
  async getVaultMeta(id: string): Promise<VaultRow | null> {
    const results = await this.client.execute(
      `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
      (row: Row) => parseVaultRow(row),
      [dbParam(id)],
    );
    return results[0] ?? null;
  }
}

/**
 * Factory function to create VaultRegistry from SharedDb client.
 */
export function createVaultRegistry(client: Client): VaultRegistry {
  return new VaultRegistry(client);
}
