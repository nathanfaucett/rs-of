import type { DatabaseHandle, ChangeEvent, SyncState, SyncOptions } from "@ofsdk/sync-sdk";
import {
  deriveKey,
  encrypt,
  decrypt,
  toBase64,
  fromBase64,
  createAuthentication,
  verifyAuthentication,
} from "$lib/vault/storage/crypto";
import { type LockSettings, LockSettingsSchema, DEFAULT_LOCK_SETTINGS, encryptLockSettings, decryptLockSettings } from "$lib/vault/lock";
import { type Secret, SecretSchema } from "$lib/vault/secrets/secret";
import { type Folder, FolderSchema } from "$lib/vault/folder";
import { type ItemMutation, validateItemSnapshot, type ItemSnapshot } from "$lib/vault/folder";
import type { VaultHandleLike } from "$lib/vault/types";
import * as v from "valibot";

interface VaultMeta {
  id: string;
  salt: string;
  iv: string;
  createdAt: string;
}

interface EncryptedSecret {
  id: string;
  ciphertext: string;
  nonce: string;
  createdAt: string;
  updatedAt: string;
}

interface SyncSDKRef {
  syncToDevice: (resourceId: string, targetDeviceId: string) => Promise<{ sessionId: string; resourceId: string; targetDevice: string; status: string }>;
}

interface SyncVaultHandleOptions {
  handle: DatabaseHandle;
  key: CryptoKey;
  meta: VaultMeta;
  syncSDK?: SyncSDKRef;
}

export class SyncVaultHandle implements VaultHandleLike {
  private key: CryptoKey;
  private meta: VaultMeta;
  private handle: DatabaseHandle;
  private prepared: ItemSnapshot;
  private syncSDK?: SyncSDKRef;

  constructor(options: SyncVaultHandleOptions) {
    this.handle = options.handle;
    this.key = options.key;
    this.meta = options.meta;
    this.syncSDK = options.syncSDK;
    this.prepared = { items: [], folders: [] };
  }

  static create(handle: DatabaseHandle, key: CryptoKey, meta: VaultMeta, syncSDK?: SyncSDKRef): SyncVaultHandle {
    return new SyncVaultHandle({ handle, key, meta, syncSDK });
  }

  get id(): string {
    return this.meta.id;
  }

  get handleId(): string {
    return this.handle.id;
  }

  get changes$() {
    return this.handle.changes$;
  }

  get state$() {
    return this.handle.state$;
  }

  async execute(sql: string, params?: unknown[]): Promise<{ columns: string[]; rows: unknown[][] }> {
    return this.handle.execute(sql, params);
  }

  async loadSecrets(): Promise<Secret[]> {
    const rows = await this.handle.execute(
      `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
      [this.meta.id],
    );

    return Promise.all(rows.rows.map(async (row: unknown[]) => {
      const secret = {
        id: row[0] as string,
        vault_id: row[1] as string,
        ciphertext: row[2] as string,
        nonce: row[3] as string,
        createdAt: row[4] as string,
        updatedAt: row[5] as string,
      } as EncryptedSecret;

      const plaintext = await decrypt(this.key, secret.ciphertext, secret.nonce);
      try {
        const parsed = v.parse(SecretSchema, JSON.parse(plaintext));
        if (parsed.id !== secret.id) {
          throw new Error("Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data.");
        }
        return parsed;
      } catch (e) {
        if (e instanceof Error && e.message.includes("Malformed vault snapshot")) throw e;
        return {
          id: secret.id,
          kind: "secureNote",
          type: 0,
          name: "",
          notes: plaintext,
          favorite: false,
          fields: [],
          history: [],
          createdAt: secret.createdAt,
          updatedAt: secret.updatedAt,
        } as Secret;
      }
    }));
  }

  async loadFolders(): Promise<Folder[]> {
    const rows = await this.handle.execute(
      `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
      [this.meta.id],
    );

    return Promise.all(rows.rows.map(async (row: unknown[]) => {
      const folderRow = {
        id: row[0] as string,
        vault_id: row[1] as string,
        ciphertext: row[2] as string,
        nonce: row[3] as string,
        createdAt: row[4] as string,
        updatedAt: row[5] as string,
      } as EncryptedSecret;

      const plaintext = await decrypt(this.key, folderRow.ciphertext, folderRow.nonce);
      const folder = v.parse(FolderSchema, JSON.parse(plaintext));
      if (folder.id !== folderRow.id) {
        throw new Error("Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data.");
      }
      return folder;
    }));
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

    await this.handle.execute("BEGIN TRANSACTION");
    try {
      if (mutation.folders?.length) {
        for (const folder of mutation.folders) {
          const encrypted = await encrypt(this.key, JSON.stringify(folder));
          await this.handle.execute(
            `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
            [folder.id, this.meta.id, encrypted.ciphertext, encrypted.nonce, new Date().toISOString(), new Date().toISOString()],
          );
        }
      }

      if (mutation.upsert?.length) {
        for (const item of mutation.upsert) {
          const encrypted = await encrypt(this.key, JSON.stringify(item));
          await this.handle.execute(
            `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
            [item.id, this.meta.id, encrypted.ciphertext, encrypted.nonce, new Date().toISOString(), new Date().toISOString()],
          );
        }
      }

      if (mutation.deleteIds?.length) {
        for (const id of mutation.deleteIds) {
          await this.handle.execute(
            `DELETE FROM secrets WHERE id = ? AND vault_id = ?`,
            [id, this.meta.id],
          );
        }
      }

      if (mutation.deleteFolderIds?.length) {
        for (const id of mutation.deleteFolderIds) {
          await this.handle.execute(
            `DELETE FROM folders WHERE id = ? AND vault_id = ?`,
            [id, this.meta.id],
          );
        }
      }

      await this.handle.execute("COMMIT");
      this.prepared = await this.itemSnapshot();
    } catch (error) {
      await this.handle.execute("ROLLBACK");
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
    const rows = await this.handle.execute(
      `SELECT ciphertext, nonce FROM secrets WHERE id = ? AND vault_id = ?`,
      [secret.id, this.meta.id],
    );
    if (!rows.rows[0]) throw new Error("Secret not found");
    const row = rows.rows[0] as unknown[];
    return decrypt(this.key, row[0] as string, row[1] as string);
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
    const encrypted = await encrypt(this.key, plaintext);
    await this.handle.execute(
      `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
       ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
      [item.id, this.meta.id, encrypted.ciphertext, encrypted.nonce, item.createdAt, item.updatedAt],
    );
    return item;
  }

  async deleteSecret(id: string): Promise<void> {
    await this.mutateItems({ deleteIds: [id] });
  }

  async exportBackup(): Promise<string> {
    const [items, folders, lockSettings, metaResult] = await Promise.all([
      this.loadSecrets(),
      this.loadFolders(),
      this.loadLockSettings(),
      this.handle.execute(
        `SELECT name, authentication FROM vaults WHERE id = ?`,
        [this.meta.id],
      ),
    ]);

    const { encodeBackup } = await import("$lib/vault/backup");
    const secretsPayload = await Promise.all(items.map(async (item) => {
      const encrypted = await encrypt(this.key, JSON.stringify(item));
      return { id: item.id, ...encrypted, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() };
    }));
    const foldersPayload = await Promise.all(folders.map(async (folder) => {
      const encrypted = await encrypt(this.key, JSON.stringify(folder));
      return { id: folder.id, ...encrypted, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() };
    }));

    const metaRow = metaResult.rows[0] as unknown[] | undefined;
    return encodeBackup(this.key, {
      meta: this.meta,
      authentication: (metaRow?.[1] as string) ?? "",
      name: (metaRow?.[0] as string) ?? "vault",
      secrets: secretsPayload,
      folders: foldersPayload,
      itemFormat: 2,
      transitionBackup: null,
      lockSettings: await encryptLockSettings(this.key, this.meta.id, lockSettings),
    });
  }

  async loadLockSettings(): Promise<LockSettings> {
    const rows = await this.handle.execute(
      `SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
      [this.meta.id],
    );
    if (!rows.rows[0]) return { ...DEFAULT_LOCK_SETTINGS };
    const row = rows.rows[0] as unknown[];
    return decryptLockSettings(this.key, this.meta.id, JSON.stringify({
      ciphertext: row[1] as string,
      nonce: row[2] as string,
    }));
  }

  async saveLockSettings(settings: LockSettings): Promise<LockSettings> {
    const encryptedJson = await encryptLockSettings(this.key, this.meta.id, settings);
    const encrypted = JSON.parse(encryptedJson);
    await this.handle.execute(
      `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)
       ON CONFLICT(vault_id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
      [this.meta.id, encrypted.ciphertext, encrypted.nonce, new Date().toISOString()],
    );
    return settings;
  }

  async sync(options?: SyncOptions): Promise<void> {
    return this.handle.sync(options);
  }

  async syncToDevice(targetDeviceId: string): Promise<void> {
    if (!this.syncSDK) {
      throw new Error("Sync SDK not available for device sync");
    }
    await this.syncSDK.syncToDevice(this.handle.id, targetDeviceId);
  }

  close(): void {
    this.handle.close();
  }
}

export async function createSyncVault(
  password: string,
  name: string,
  syncHandle: DatabaseHandle,
  syncSDK?: SyncSDKRef,
): Promise<SyncVaultHandle> {
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

  // Insert vault metadata
  await syncHandle.execute(
    `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
    [vaultId, name.trim(), meta.salt, meta.iv, meta.createdAt, authentication],
  );

  return SyncVaultHandle.create(syncHandle, key, meta, syncSDK);
}

export async function unlockSyncVault(
  password: string,
  vaultId: string,
  syncHandle: DatabaseHandle,
  syncSDK?: SyncSDKRef,
): Promise<SyncVaultHandle | null> {
  const results = await syncHandle.execute(
    `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
    [vaultId],
  );

  if (!results.rows[0]) return null;
  const row = results.rows[0] as unknown[];
  const meta = {
    id: row[0] as string,
    name: row[1] as string,
    salt: row[2] as string,
    iv: row[3] as string,
    createdAt: row[4] as string,
    authentication: row[5] as string | null,
    status: row[6] as string,
    migrated: row[7] as number,
  };

  const salt = fromBase64(meta.salt);
  const key = await deriveKey(password, salt);
  if (meta.authentication) {
    await verifyAuthentication(key, { id: meta.id, salt: meta.salt, iv: meta.iv, createdAt: meta.createdAt }, meta.authentication);
  } else {
    const secrets = await syncHandle.execute(
      `SELECT id, ciphertext, nonce FROM secrets WHERE vault_id = ?`,
      [vaultId],
    );
    if (secrets.rows.length === 0) {
      throw new Error("original password cannot be verified");
    }
    for (const s of secrets.rows) {
      const secretRow = s as unknown[];
      try {
        await decrypt(key, secretRow[1] as string, secretRow[2] as string);
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

  return SyncVaultHandle.create(syncHandle, key, vaultMeta, syncSDK);
}

export async function listSyncVaults(syncHandle: DatabaseHandle): Promise<
  { id: string; name: string; status: "ready"; migrated: boolean }[]
> {
  const results = await syncHandle.execute(
    `SELECT id, name, status, migrated FROM vaults WHERE status = 'ready' ORDER BY created_at DESC`,
    [],
  );
  return results.rows.map((row: unknown[]) => ({
    id: row[0] as string,
    name: row[1] as string,
    status: "ready" as const,
    migrated: Boolean(row[3]),
  }));
}
