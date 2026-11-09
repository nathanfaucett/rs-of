import type { LockSettings } from "$lib/vault/lock";
import type { Secret } from "$lib/vault/secrets/secret";
import type { Folder } from "$lib/vault/folder";
import type { ItemMutation, ItemSnapshot } from "$lib/vault/folder";

export interface VaultHandleLike {
  /** Unique vault identifier */
  readonly id: string;

  /** Load all secrets in the vault */
  loadSecrets(): Promise<Secret[]>;

  /** Load all folders in the vault */
  loadFolders(): Promise<Folder[]>;

  /** Read current items snapshot (secrets + folders) */
  readItems(): Promise<ItemSnapshot>;

  /** Prepare items for editing (same as readItems for now) */
  prepareItems(): Promise<ItemSnapshot>;

  /** Apply mutations to items */
  mutateItems(mutation: ItemMutation): Promise<void>;

  /** Create a new folder */
  createFolder(name: string): Promise<Folder>;

  /** Rename a folder */
  renameFolder(folderId: string, name: string): Promise<void>;

  /** Move items to a folder */
  moveItems(itemIds: string[], folderId: string | null): Promise<void>;

  /** Delete a folder */
  deleteFolder(folderId: string): Promise<void>;

  /** Decrypt a secret's plaintext */
  decryptSecret(secret: Secret): Promise<string>;

  /** Encrypt and store a plaintext secret */
  encryptAndStore(plaintext: string): Promise<Secret>;

  /** Delete a secret */
  deleteSecret(id: string): Promise<void>;

  /** Export encrypted backup */
  exportBackup(): Promise<string>;

  /** Export pre-transition backup (for recovery) */
  exportTransitionBackup?(): Promise<string>;

  /** Load lock settings */
  loadLockSettings(): Promise<LockSettings>;

  /** Save lock settings */
  saveLockSettings(settings: LockSettings): Promise<LockSettings>;

  /** Sync vault with remote (if available) */
  sync?(options?: { direction?: "bidirectional" | "push" | "pull"; conflictStrategy?: "local-wins" | "remote-wins" | "merge" | "manual" }): Promise<void>;

  /** Sync vault to another device (if available) */
  syncToDevice?(targetDeviceId: string): Promise<void>;

  /** Close the vault */
  close(): void;
}

export type { LockSettings, Secret, Folder, ItemMutation, ItemSnapshot };
