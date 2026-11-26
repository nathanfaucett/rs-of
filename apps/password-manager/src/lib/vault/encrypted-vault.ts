import type { RowParser } from "@ofsdk/sql";
import * as v from "valibot";
import {
  type Folder,
  FolderSchema,
  type ItemMutation,
  type ItemSnapshot,
  validateItemSnapshot,
} from "./folder";
import { decryptLockSettings, encryptLockSettings, type LockSettings } from "./lock";
import { type Secret, SecretSchema } from "./secrets/secret";
import {
  createAuthentication,
  decrypt,
  deriveKey,
  encrypt,
  fromBase64,
  toBase64,
  type VaultMeta,
  verifyAuthentication,
} from "./storage/crypto";
import { type EncryptedRowParsers, parseOptionalText, parseText } from "./storage/schema";
import type { VaultHandleLike } from "./types";

type VaultAuthentication = VaultMeta & { authentication?: string | null | undefined };

interface VaultWrite {
  sql: string;
  params: string[];
}

export interface VaultDatabase {
  read<T>(sql: string, parse: RowParser<T>, params: string[]): Promise<T[]>;
  write(sql: string, params: string[]): Promise<void>;
  writeBatch(statements: VaultWrite[]): Promise<void>;
  parseMeta: RowParser<VaultAuthentication>;
  encryptedRows: EncryptedRowParsers;
  verifyExists?(id: string): Promise<void>;
  withItemLock?<T>(id: string, action: () => Promise<T>): Promise<T>;
  sync?: VaultHandleLike["sync"];
  close(): Promise<void>;
}

export class VaultHandle implements VaultHandleLike {
  readonly sync?: NonNullable<VaultHandleLike["sync"]>;
  private readonly database: VaultDatabase;
  private readonly key: CryptoKey;
  private readonly meta: VaultMeta;

  constructor(database: VaultDatabase, key: CryptoKey, meta: VaultMeta) {
    this.database = database;
    this.key = key;
    this.meta = meta;
    if (database.sync) this.sync = database.sync;
  }

  get id(): string {
    return this.meta.id;
  }

  private withItemLock<T>(action: () => Promise<T>): Promise<T> {
    return this.database.withItemLock ? this.database.withItemLock(this.id, action) : action();
  }

  async loadSecrets(): Promise<Secret[]> {
    return this.withItemLock(async () => {
      await this.database.verifyExists?.(this.id);
      const rows = await this.database.read(
        `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
        this.database.encryptedRows.parseItemRow,
        [this.id]
      );
      return Promise.all(
        rows.map(async (secret) => {
          const plaintext = await decrypt(this.key, secret.ciphertext, secret.nonce);
          try {
            const parsed = v.parse(SecretSchema, JSON.parse(plaintext));
            if (parsed.id !== secret.id) {
              throw new Error(
                "Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data."
              );
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
        })
      );
    });
  }

  async loadFolders(): Promise<Folder[]> {
    return this.withItemLock(async () => {
      await this.database.verifyExists?.(this.id);
      const rows = await this.database.read(
        `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
        this.database.encryptedRows.parseItemRow,
        [this.id]
      );
      return Promise.all(
        rows.map(async (folderRow) => {
          const plaintext = await decrypt(this.key, folderRow.ciphertext, folderRow.nonce);
          const folder = v.parse(FolderSchema, JSON.parse(plaintext));
          if (folder.id !== folderRow.id) {
            throw new Error(
              "Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data."
            );
          }
          return folder;
        })
      );
    });
  }

  private async itemSnapshot(): Promise<ItemSnapshot> {
    const [items, folders] = await Promise.all([this.loadSecrets(), this.loadFolders()]);
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

    const statements: VaultWrite[] = [];
    for (const folder of mutation.folders ?? []) {
      const now = new Date().toISOString();
      statements.push(
        await this.encryptedWrite("folders", folder.id, JSON.stringify(folder), now, now)
      );
    }
    for (const item of mutation.upsert ?? []) {
      const now = new Date().toISOString();
      statements.push(
        await this.encryptedWrite("secrets", item.id, JSON.stringify(item), now, now)
      );
    }
    for (const id of mutation.deleteIds ?? []) {
      statements.push({
        sql: "DELETE FROM secrets WHERE id = ? AND vault_id = ?",
        params: [id, this.id],
      });
    }
    for (const id of mutation.deleteFolderIds ?? []) {
      statements.push({
        sql: "DELETE FROM folders WHERE id = ? AND vault_id = ?",
        params: [id, this.id],
      });
    }
    await this.database.writeBatch(statements);
  }

  private async encryptedWrite(
    table: "secrets" | "folders",
    id: string,
    plaintext: string,
    createdAt: string,
    updatedAt: string
  ): Promise<VaultWrite> {
    const { ciphertext, nonce } = await encrypt(this.key, plaintext);
    return {
      sql: `INSERT INTO ${table} (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
      params: [id, this.id, ciphertext, nonce, createdAt, updatedAt],
    };
  }

  async createFolder(name: string): Promise<Folder> {
    const folder: Folder = { id: crypto.randomUUID(), name };
    await this.mutateItems({ folders: [folder] });
    return folder;
  }

  async renameFolder(folderId: string, name: string): Promise<void> {
    const folders = (await this.loadFolders()).map((f) => (f.id === folderId ? { ...f, name } : f));
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
    const rows = await this.database.read(
      `SELECT ciphertext, nonce FROM secrets WHERE id = ? AND vault_id = ?`,
      (row) => ({
        ciphertext: parseText(row, "ciphertext"),
        nonce: parseText(row, "nonce"),
      }),
      [secret.id, this.id]
    );
    if (!rows[0]) throw new Error("Secret not found");
    return decrypt(this.key, rows[0].ciphertext, rows[0].nonce);
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
    const { sql, params } = await this.encryptedWrite(
      "secrets",
      item.id,
      plaintext,
      item.createdAt,
      item.updatedAt
    );
    await this.database.write(sql, params);
    return item;
  }

  async deleteSecret(id: string): Promise<void> {
    await this.mutateItems({ deleteIds: [id] });
  }

  async exportBackup(): Promise<string> {
    await this.database.verifyExists?.(this.id);
    const [items, folders, lockSettings, metaResult] = await Promise.all([
      this.loadSecrets(),
      this.loadFolders(),
      this.loadLockSettings(),
      this.database.read(
        `SELECT name, authentication FROM vaults WHERE id = ?`,
        (row) => ({
          name: parseOptionalText(row, "name") ?? "vault",
          authentication: parseOptionalText(row, "authentication"),
        }),
        [this.id]
      ),
    ]);

    const { encodeBackup } = await import("./backup");
    const secretsPayload = await Promise.all(
      items.map(async (item) => {
        const encrypted = await encrypt(this.key, JSON.stringify(item));
        return {
          id: item.id,
          ...encrypted,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };
      })
    );
    const foldersPayload = await Promise.all(
      folders.map(async (folder) => {
        const encrypted = await encrypt(this.key, JSON.stringify(folder));
        return {
          id: folder.id,
          ...encrypted,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };
      })
    );

    return encodeBackup(this.key, {
      meta: this.meta,
      authentication: metaResult[0]?.authentication ?? "",
      name: metaResult[0]?.name ?? "vault",
      secrets: secretsPayload,
      folders: foldersPayload,
      itemFormat: 2,
      transitionBackup: null,
      lockSettings: await encryptLockSettings(this.key, this.id, lockSettings),
    });
  }

  async loadLockSettings(): Promise<LockSettings> {
    return this.withItemLock(async () => {
      await this.database.verifyExists?.(this.id);
      const results = await this.database.read(
        `SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
        this.database.encryptedRows.parseLockSettingsRow,
        [this.id]
      );
      const saved = results[0];
      return decryptLockSettings(
        this.key,
        this.id,
        saved ? JSON.stringify({ ciphertext: saved.ciphertext, nonce: saved.nonce }) : null
      );
    });
  }

  async saveLockSettings(settings: LockSettings): Promise<LockSettings> {
    return this.withItemLock(async () => {
      await this.database.verifyExists?.(this.id);
      const encryptedJson = await encryptLockSettings(this.key, this.id, settings);
      const encrypted = JSON.parse(encryptedJson);
      await this.database.write(
        `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)
           ON CONFLICT(vault_id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
        [this.id, encrypted.ciphertext, encrypted.nonce, new Date().toISOString()]
      );
      return settings;
    });
  }

  close(): Promise<void> {
    return this.database.close();
  }
}

export async function createEncryptedVault(
  database: VaultDatabase,
  password: string,
  name: string
): Promise<VaultHandle> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt.buffer);
  const meta: VaultMeta = {
    id: crypto.randomUUID(),
    salt: toBase64(salt),
    iv: toBase64(iv),
    createdAt: new Date().toISOString(),
  };
  const authentication = await createAuthentication(key, meta);
  await database.writeBatch([
    {
      sql: `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
      params: [meta.id, name.trim(), meta.salt, meta.iv, meta.createdAt, authentication],
    },
  ]);
  return new VaultHandle(database, key, meta);
}

export async function unlockEncryptedVault(
  database: VaultDatabase,
  password: string,
  id: string
): Promise<VaultHandle | null> {
  const [row] = await database.read(
    `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
    database.parseMeta,
    [id]
  );
  if (!row) return null;

  const { id: vaultId, salt, iv, createdAt, authentication } = row;
  const meta = { id: vaultId, salt, iv, createdAt };
  const key = await deriveKey(password, fromBase64(salt));
  if (authentication) {
    await verifyAuthentication(key, meta, authentication);
  } else {
    // A legacy vault without authentication proof must decrypt every record.
    const secrets = await database.read(
      `SELECT ciphertext, nonce FROM secrets WHERE vault_id = ?`,
      (row) => ({ ciphertext: parseText(row, "ciphertext"), nonce: parseText(row, "nonce") }),
      [id]
    );
    if (secrets.length === 0) throw new Error("original password cannot be verified");
    for (const { ciphertext, nonce } of secrets) {
      try {
        await decrypt(key, ciphertext, nonce);
      } catch {
        throw new Error("Wrong password or damaged legacy records");
      }
    }
  }
  return new VaultHandle(database, key, meta);
}
