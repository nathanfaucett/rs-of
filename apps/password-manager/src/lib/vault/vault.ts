import { Client, Transaction, type Row } from "@ofsdk/sql";
import { migrateToSharedTables, needsMigration } from "./migrate-to-shared";
import * as v from "valibot";
import { SharedDb } from "./SharedDb";
import { VaultRegistry, createVaultRegistry } from "./VaultRegistry";
import {
  parseSecretRow,
  parseFolderRow,
  parseLockSettingsRow,
  parseVaultRow,
  parseOptionalText,
  parseText,
  dbParams,
  dbParam,
} from "./schema";
import {
  deriveKey,
  encrypt,
  decrypt,
  toBase64,
  fromBase64,
  createAuthentication,
  verifyAuthentication,
} from "./crypto";
import { type LockSettings, LockSettingsSchema, DEFAULT_LOCK_SETTINGS, encryptLockSettings, decryptLockSettings } from "./lock";
import { type Secret, SecretSchema } from "../secrets/secret";
import { type Folder, FolderSchema } from "./folder";
import { type ItemMutation, validateItemSnapshot, type ItemSnapshot } from "./folder";

interface VaultMeta {
  id: string;
  salt: string;
  iv: string;
  createdAt: string;
}

export interface EncryptedSecret {
  id: string;
  ciphertext: string;
  nonce: string;
  createdAt: string;
  updatedAt: string;
}

class VaultHandle {
  private key: CryptoKey;
  private prepared: ItemSnapshot;
  private vaultId: string;
  private client: Client;
  private meta: VaultMeta;

  private constructor(
    key: CryptoKey,
    client: Client,
    meta: VaultMeta,
    vaultId: string,
  ) {
    this.key = key;
    this.client = client;
    this.meta = meta;
    this.vaultId = vaultId;
    this.prepared = { items: [], folders: [] };
  }

  static create(
    key: CryptoKey,
    client: Client,
    meta: VaultMeta,
    vaultId: string,
  ): VaultHandle {
    return new VaultHandle(key, client, meta, vaultId);
  }

  private activeKey(): CryptoKey {
    return this.key;
  }

  get id(): string {
    return this.vaultId;
  }

  async loadSecrets(): Promise<Secret[]> {
    return navigator.locks.request(
      `password-manager-items-${this.vaultId}`,
      async () => {
        await this.verifyVaultExists();
        const rows = await this.client.execute(
          `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
          (row: Row) => parseSecretRow(row),
          [dbParam(this.vaultId)],
        );
        return Promise.all(rows.map(async (secret) => {
          const plaintext = await decrypt(this.activeKey(), secret.ciphertext, secret.nonce);
          try {
            const parsed = v.parse(SecretSchema, JSON.parse(plaintext));
            if (parsed.id !== secret.id) {
              throw new Error("Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data.");
            }
            return parsed;
          } catch (e) {
            if (e instanceof Error && e.message.includes("Malformed vault snapshot")) throw e;
            return { id: secret.id, kind: "secureNote", type: 0, name: "", notes: plaintext, favorite: false, fields: [], history: [], createdAt: secret.createdAt, updatedAt: secret.updatedAt } as Secret;
          }
        }));
      },
    );
  }

  async loadFolders(): Promise<Folder[]> {
    return navigator.locks.request(
      `password-manager-items-${this.vaultId}`,
      async () => {
        await this.verifyVaultExists();
        const rows = await this.client.execute(
          `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
          (row: Row) => parseFolderRow(row),
          [dbParam(this.vaultId)],
        );
        return Promise.all(rows.map(async (folderRow) => {
          const plaintext = await decrypt(this.activeKey(), folderRow.ciphertext, folderRow.nonce);
          const folder = v.parse(FolderSchema, JSON.parse(plaintext));
          if (folder.id !== folderRow.id) {
            throw new Error("Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data.");
          }
          return folder;
        }));
      },
    );
  }

  private async itemSnapshot(): Promise<ItemSnapshot> {
    const [items, folders] = await Promise.all([
      this.loadSecrets(),
      this.loadFolders(),
    ]);
    return { items, folders };
  }

  async readItems(): Promise<ItemSnapshot> {
    return this.itemSnapshot();
  }

  async prepareItems(): Promise<ItemSnapshot> {
    return this.readItems();
  }

  async mutateItems(mutation: ItemMutation): Promise<void> {
    const current = await this.itemSnapshot();
    validateItemSnapshot(current);

    const tx = await this.client.transaction();
    try {
      // Handle folder upserts - folders encrypted same as secrets
      if (mutation.folders?.length) {
        for (const folder of mutation.folders) {
          const encrypted = await encrypt(this.activeKey(), JSON.stringify(folder));
          await tx.executeRaw(
            `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
            dbParams(
              folder.id,
              this.vaultId,
              encrypted.ciphertext,
              encrypted.nonce,
              new Date().toISOString(),
              new Date().toISOString(),
            ),
          );
        }
      }

      // Handle secret upserts
      if (mutation.upsert?.length) {
        for (const item of mutation.upsert) {
          const encrypted = await encrypt(this.activeKey(), JSON.stringify(item));
          await tx.executeRaw(
            `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
            dbParams(
              item.id,
              this.vaultId,
              encrypted.ciphertext,
              encrypted.nonce,
              new Date().toISOString(),
              new Date().toISOString(),
            ),
          );
        }
      }

      // Handle secret deletes
      if (mutation.deleteIds?.length) {
        for (const id of mutation.deleteIds) {
          await tx.executeRaw(
            `DELETE FROM secrets WHERE id = ? AND vault_id = ?`,
            [dbParam(id), dbParam(this.vaultId)],
          );
        }
      }

      // Handle folder deletes
      if (mutation.deleteFolderIds?.length) {
        for (const id of mutation.deleteFolderIds) {
          await tx.executeRaw(
            `DELETE FROM folders WHERE id = ? AND vault_id = ?`,
            [dbParam(id), dbParam(this.vaultId)],
          );
        }
      }

      await tx.commit();
      this.prepared = await this.itemSnapshot();
    } catch (error) {
      await tx.rollback();
      throw error;
    }
  }

  async createFolder(name: string): Promise<Folder> {
    const folder: Folder = { id: crypto.randomUUID(), name };
    await this.mutateItems({ folders: [folder] });
    return folder;
  }

  async renameFolder(folderId: string, name: string): Promise<void> {
    const folders = (await this.loadFolders()).map((f) =>
      f.id === folderId ? { ...f, name } : f,
    );
    await this.mutateItems({ folders });
  }

  async moveItems(itemIds: string[], folderId: string | null): Promise<void> {
    const items = await this.loadSecrets();
    const moving = items.filter((item) => itemIds.includes(item.id));
    if (moving.length !== itemIds.length) {
      throw new Error("Some items not found");
    }
    const updated = moving.map((item) => ({ ...item, folderId }));
    await this.mutateItems({ upsert: updated });
  }

  async deleteFolder(folderId: string): Promise<void> {
    await this.mutateItems({ deleteFolderIds: [folderId] });
  }

  async decryptSecret(secret: Secret): Promise<string> {
    const rows = await this.client.execute(
      `SELECT ciphertext, nonce FROM secrets WHERE id = ? AND vault_id = ?`,
      (row: Row) => ({
        ciphertext: parseText(row, "ciphertext"),
        nonce: parseText(row, "nonce"),
      }),
      [dbParam(secret.id), dbParam(this.vaultId)],
    );
    if (!rows[0]) throw new Error("Secret not found");
    return decrypt(this.activeKey(), rows[0].ciphertext, rows[0].nonce);
  }

  async encryptAndStore(plaintext: string): Promise<Secret> {
    let item: Secret;
    try {
      item = v.parse(SecretSchema, JSON.parse(plaintext));
    } catch {
      item = {
        id: crypto.randomUUID(),
        kind: "secureNote",
        type: 0,
        name: plaintext.slice(0, 50),
        notes: plaintext,
        favorite: false,
        fields: [],
        history: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      } as Secret;
    }
    const encrypted = await encrypt(this.activeKey(), plaintext);
    await this.client.executeRaw(
      `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
       ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
      dbParams(item.id, this.vaultId, encrypted.ciphertext, encrypted.nonce, item.createdAt, item.updatedAt),
    );
    return item;
  }

  async deleteSecret(id: string): Promise<void> {
    await this.mutateItems({ deleteIds: [id] });
  }

  async exportBackup(): Promise<string> {
    await this.verifyVaultExists();
    const [items, folders, lockSettings, metaResult] = await Promise.all([
      this.loadSecrets(),
      this.loadFolders(),
      this.loadLockSettings(),
      this.client.execute(
        `SELECT name, authentication FROM vaults WHERE id = ?`,
        (row: Row) => ({
          name: row.name && row.name.type !== "null" ? (row.name as { value: string }).value : "vault",
          authentication: row.authentication?.type === "null" || !row.authentication ? null : (row.authentication as { value: string }).value,
        }),
        [dbParam(this.vaultId)],
      ),
    ]);

    const { encodeBackup } = await import("./backup");
    const secretsPayload = await Promise.all(items.map(async (item) => {
      const encrypted = await encrypt(this.activeKey(), JSON.stringify(item));
      return { id: item.id, ...encrypted, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() };
    }));
    const foldersPayload = await Promise.all(folders.map(async (folder) => {
      const encrypted = await encrypt(this.activeKey(), JSON.stringify(folder));
      return { id: folder.id, ...encrypted, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() };
    }));

    return encodeBackup(this.activeKey(), {
      meta: this.meta,
      authentication: metaResult[0]?.authentication ?? "",
      name: metaResult[0]?.name ?? "vault",
      secrets: secretsPayload,
      folders: foldersPayload,
      itemFormat: 2,
      transitionBackup: null,
      lockSettings: await encryptLockSettings(this.activeKey(), this.vaultId, lockSettings),
    });
  }

  async loadLockSettings(): Promise<LockSettings> {
    return navigator.locks.request(
      `password-manager-items-${this.vaultId}`,
      async () => {
        await this.verifyVaultExists();
        const results = await this.client.execute(
          `SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
          (row: Row) => parseLockSettingsRow(row),
          [dbParam(this.vaultId)],
        );
        if (!results[0]) return { ...DEFAULT_LOCK_SETTINGS };
        return decryptLockSettings(this.activeKey(), this.vaultId, JSON.stringify({
          ciphertext: results[0].ciphertext,
          nonce: results[0].nonce,
        }));
      },
    );
  }

  async saveLockSettings(settings: LockSettings): Promise<LockSettings> {
    return navigator.locks.request(
      `password-manager-items-${this.vaultId}`,
      async () => {
        await this.verifyVaultExists();
        const encryptedJson = await encryptLockSettings(this.activeKey(), this.vaultId, settings);
        const encrypted = JSON.parse(encryptedJson);
        await this.client.executeRaw(
          `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)
           ON CONFLICT(vault_id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
          dbParams(this.vaultId, encrypted.ciphertext, encrypted.nonce, new Date().toISOString()),
        );
        return settings;
      },
    );
  }

  private async verifyVaultExists(): Promise<void> {
    const identity = await createVaultRegistry(this.client).findVault(this.vaultId);
    if (!identity) throw new Error("Vault not found");
  }

  close(): void {
    // Client is managed by SharedDb singleton
  }
}

export async function createVault(
  password: string,
  name: string,
): Promise<VaultHandle> {
  const trimmedName = name.trim();
  const vaultId = crypto.randomUUID();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt.buffer);
  const meta: VaultMeta = {
    id: vaultId,
    salt: toBase64(salt),
    iv: toBase64(iv),
    createdAt: new Date().toISOString(),
  };
  const authentication = await createAuthentication(key, meta);

  const client = await SharedDb.getInstance().getClient();

  // Single transaction: insert vault metadata
  const tx = await client.transaction();
  try {
    await tx.executeRaw(
      `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
      dbParams(vaultId, trimmedName, meta.salt, meta.iv, meta.createdAt, authentication),
    );
    await tx.commit();
  } catch (e) {
    await tx.rollback();
    throw e;
  }

  return VaultHandle.create(key, client, meta, vaultId);
}

export async function unlockVault(
  password: string,
  id: string,
): Promise<VaultHandle> {
  // Run one-time migration from legacy per-vault DBs + registry to shared tables
  if (await needsMigration()) {
    await migrateToSharedTables();
  }

  const client = await SharedDb.getInstance().getClient();

  // Read from vaults table using typed execute
  const results = await client.execute(
    `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
    (row: Row) => parseVaultRow(row),
    [dbParam(id)],
  );

  if (!results[0]) throw new Error("Vault not found");
  const meta = results[0];

  const salt = fromBase64(meta.salt);
  const key = await deriveKey(password, salt);
  if (meta.authentication) {
    await verifyAuthentication(key, meta, meta.authentication);
  } else {
    // Legacy vault without authentication proof: verify by decrypting ALL records
    const secrets = await client.execute(
      `SELECT id, ciphertext, nonce FROM secrets WHERE vault_id = ?`,
      (row: Row) => ({
        id: parseText(row, "id"),
        ciphertext: parseText(row, "ciphertext"),
        nonce: parseText(row, "nonce"),
      }),
      [dbParam(id)],
    );
    if (secrets.length === 0) {
      throw new Error("original password cannot be verified");
    }
    for (const s of secrets) {
      try {
        await decrypt(key, s.ciphertext, s.nonce);
      } catch {
        throw new Error("Wrong password or damaged legacy records");
      }
    }
  }

  const vaultMeta: VaultMeta = {
    id: meta.id,
    salt: meta.salt,
    iv: meta.iv,
    createdAt: meta.createdAt,
  };

  return VaultHandle.create(key, client, vaultMeta, meta.id);
}

export async function listVaults(): Promise<
  { id: string; name: string; status: "ready"; migrated: boolean }[]
> {
  const client = await SharedDb.getInstance().getClient();
  return createVaultRegistry(client).listVaultIdentities();
}

export type { VaultHandle };
export type { VaultMeta };
export interface VaultIdentity {
  id: string;
  name: string;
  status: "ready";
  migrated: boolean;
}
