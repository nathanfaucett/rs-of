import * as v from "valibot";
import type { DbValue } from "@ofsdk/sql";

/**
 * Shared table DDL statements for the new architecture.
 * All tables in single shared DB (password-manager-v1).
 * Vault ownership via vault_id FK columns.
 */

export const CREATE_VAULTS_TABLE = `
CREATE TABLE IF NOT EXISTS vaults (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  salt TEXT NOT NULL,        -- base64, 16 bytes
  iv TEXT NOT NULL,          -- base64, 12 bytes
  created_at TEXT NOT NULL,
  authentication TEXT,       -- JSON { version, ciphertext, nonce }
  status TEXT NOT NULL DEFAULT 'ready',  -- 'ready' only (no 'pending')
  migrated INTEGER NOT NULL DEFAULT 1    -- legacy migration flag
)
`;

export const CREATE_LOCK_SETTINGS_TABLE = `
CREATE TABLE IF NOT EXISTS lock_settings (
  vault_id TEXT PRIMARY KEY,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  updated_at TEXT NOT NULL
)
`;

export const CREATE_SECRETS_TABLE = `
CREATE TABLE IF NOT EXISTS secrets (
  id TEXT PRIMARY KEY,
  vault_id TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_secrets_vault ON secrets(vault_id)
`;

export const CREATE_FOLDERS_TABLE = `
CREATE TABLE IF NOT EXISTS folders (
  id TEXT PRIMARY KEY,
  vault_id TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_folders_vault ON folders(vault_id)
`;

/**
 * Ordered migration list for client.migrate().
 * Applied once on fresh install or after upgrade.
 */
export const MIGRATIONS = [
  {
    version: 1,
    description: "Create vaults table (registry)",
    sql: CREATE_VAULTS_TABLE,
  },
  {
    version: 2,
    description: "Create lock_settings table",
    sql: CREATE_LOCK_SETTINGS_TABLE,
  },
  {
    version: 3,
    description: "Create secrets table",
    sql: CREATE_SECRETS_TABLE,
  },
  {
    version: 4,
    description: "Create folders table",
    sql: CREATE_FOLDERS_TABLE,
  },
] as const satisfies readonly {
  version: number;
  description: string;
  sql: string;
}[];

/**
 * Valibot schemas for typed execute() parsers.
 * Each maps a row from the corresponding table.
 */

// Helper to parse tagged DbValue from Row
export function parseText(row: Record<string, DbValue>, key: string): string {
  const cell = row[key];
  if (!cell || (cell.type !== "text" && cell.type !== "uuid")) {
    throw new Error(`Expected text/uuid for ${key}, got ${cell?.type}`);
  }
  return cell.value;
}

export function parseInteger(row: Record<string, DbValue>, key: string): number {
  const cell = row[key];
  if (!cell || cell.type !== "integer") {
    throw new Error(`Expected integer for ${key}, got ${cell?.type}`);
  }
  return parseInt(cell.value, 10);
}

export function parseOptionalText(row: Record<string, DbValue>, key: string): string | null {
  const cell = row[key];
  if (!cell || cell.type === "null") return null;
  if (cell.type !== "text" && cell.type !== "uuid") {
    throw new Error(`Expected text/uuid/null for ${key}, got ${cell?.type}`);
  }
  return cell.value;
}

/**
 * Vault row from vaults table
 */
export const VaultRowSchema = v.object({
  id: v.pipe(v.string(), v.uuid()),
  name: v.pipe(v.string(), v.minLength(1)),
  salt: v.pipe(v.string(), v.minLength(1)),
  iv: v.pipe(v.string(), v.minLength(1)),
  createdAt: v.pipe(v.string(), v.isoTimestamp()),
  authentication: v.nullish(v.pipe(v.string(), v.minLength(1))),
  status: v.literal("ready"),
  migrated: v.boolean(),
});

export type VaultRow = v.InferOutput<typeof VaultRowSchema>;

/**
 * Secret row from secrets table
 */
export const SecretRowSchema = v.object({
  id: v.string(),
  vaultId: v.string(),
  ciphertext: v.pipe(v.string(), v.minLength(1)),
  nonce: v.pipe(v.string(), v.minLength(1)),
  createdAt: v.pipe(v.string(), v.isoTimestamp()),
  updatedAt: v.pipe(v.string(), v.isoTimestamp()),
});

export type SecretRow = v.InferOutput<typeof SecretRowSchema>;

/**
 * Folder row from folders table
 */
export const FolderRowSchema = v.object({
  id: v.string(),
  vaultId: v.string(),
  ciphertext: v.pipe(v.string(), v.minLength(1)),
  nonce: v.pipe(v.string(), v.minLength(1)),
  createdAt: v.pipe(v.string(), v.isoTimestamp()),
  updatedAt: v.pipe(v.string(), v.isoTimestamp()),
});

export type FolderRow = v.InferOutput<typeof FolderRowSchema>;

/**
 * Lock settings row from lock_settings table
 */
export const LockSettingsRowSchema = v.object({
  vaultId: v.string(),
  ciphertext: v.pipe(v.string(), v.minLength(1)),
  nonce: v.pipe(v.string(), v.minLength(1)),
  updatedAt: v.pipe(v.string(), v.isoTimestamp()),
});

export type LockSettingsRow = v.InferOutput<typeof LockSettingsRowSchema>;

/**
 * Parser functions for typed execute().
 * These convert null-prototype Row with tagged DbValue cells to typed objects.
 */

export function parseVaultRow(row: Record<string, DbValue>): VaultRow {
  return v.parse(VaultRowSchema, {
    id: parseText(row, "id"),
    name: parseText(row, "name"),
    salt: parseText(row, "salt"),
    iv: parseText(row, "iv"),
    createdAt: parseText(row, "created_at"),
    authentication: parseOptionalText(row, "authentication"),
    status: "ready",
    migrated: parseInteger(row, "migrated") === 1,
  });
}

export function parseSecretRow(row: Record<string, DbValue>): SecretRow {
  return v.parse(SecretRowSchema, {
    id: parseText(row, "id"),
    vaultId: parseText(row, "vault_id"),
    ciphertext: parseText(row, "ciphertext"),
    nonce: parseText(row, "nonce"),
    createdAt: parseText(row, "created_at"),
    updatedAt: parseText(row, "updated_at"),
  });
}

export function parseFolderRow(row: Record<string, DbValue>): FolderRow {
  return v.parse(FolderRowSchema, {
    id: parseText(row, "id"),
    vaultId: parseText(row, "vault_id"),
    ciphertext: parseText(row, "ciphertext"),
    nonce: parseText(row, "nonce"),
    createdAt: parseText(row, "created_at"),
    updatedAt: parseText(row, "updated_at"),
  });
}

export function parseLockSettingsRow(row: Record<string, DbValue>): LockSettingsRow {
  return v.parse(LockSettingsRowSchema, {
    vaultId: parseText(row, "vault_id"),
    ciphertext: parseText(row, "ciphertext"),
    nonce: parseText(row, "nonce"),
    updatedAt: parseText(row, "updated_at"),
  });
}

/**
 * Helper to create DbValue parameter arrays for execute()
 */
export function dbParams(...values: string[]): Array<{ type: "text"; value: string }> {
  return values.map((value) => ({ type: "text", value }));
}

export function dbParam(value: string): { type: "text"; value: string } {
  return { type: "text", value };
}

export function dbInteger(value: number): { type: "integer"; value: string } {
  return { type: "integer", value: String(value) };
}
