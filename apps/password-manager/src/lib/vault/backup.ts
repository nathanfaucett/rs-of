import { Client, type Row, Transaction } from "@ofsdk/sql";
import { SharedDb } from "./storage/SharedDb";
import { VaultRegistry, createVaultRegistry, type VaultIdentity } from "./storage/VaultRegistry";
import * as v from "valibot";
import { transitionLogin } from "./secrets/legacy-login";
import { type Secret, SecretSchema } from "./secrets/secret";
import {
  decrypt,
  deriveKey,
  encrypt,
  fromBase64,
  PBKDF2_ITERATIONS,
  toBase64,
  verifyAuthentication,
} from "./storage/crypto";
import { type Folder, FolderSchema, validateItemSnapshot } from "./folder";
import { decryptLockSettings, encryptLockSettings, type LockSettings } from "./lock";
import {
  parseSecretRow,
  parseFolderRow,
  parseVaultRow,
  parseLockSettingsRow,
  dbParams,
  dbParam,
} from "./storage/schema";

export const BACKUP_MAX_BYTES = 16 * 1024 * 1024;
const MAX_RECORDS = 10_000;
const FORMAT = "password-manager/encrypted-backup";
const invalid = () =>
  new Error(
    "Invalid or unsupported encrypted backup. Keep the original file and source vault. Use a complete supported backup within the 16 MiB / 10,000 record limit.",
  );
const text = v.pipe(v.string(), v.minLength(1));
const date = v.pipe(v.string(), v.isoTimestamp());
const uuid = v.pipe(v.string(), v.uuid());
const metaSchema = v.strictObject({
  id: uuid,
  salt: text,
  iv: text,
  createdAt: date,
});
const recordSchema = v.strictObject({
  id: uuid,
  ciphertext: text,
  nonce: text,
  createdAt: date,
  updatedAt: date,
});
const payloadSchema = v.strictObject({
  folders: v.optional(
    v.pipe(v.array(recordSchema), v.maxLength(MAX_RECORDS)),
  ),

  lockSettings: v.optional(v.nullable(text)),
  itemFormat: v.optional(v.literal(2)),
  transitionBackup: v.optional(v.nullable(text)),
  meta: metaSchema,
  authentication: text,
  name: v.pipe(
    text,
    v.check((name) => !!name.trim()),
  ),
  secrets: v.pipe(v.array(recordSchema), v.maxLength(MAX_RECORDS)),
});
type BackupPayload = v.InferOutput<typeof payloadSchema>;
const envelopeSchema = v.strictObject({
  format: v.literal(FORMAT),
  version: v.picklist([1, 2, 3]),
  kdf: v.strictObject({
    name: v.literal("PBKDF2"),
    hash: v.literal("SHA-512"),
    iterations: v.literal(PBKDF2_ITERATIONS),
    salt: text,
  }),
  cipher: v.literal("AES-256-GCM"),
  nonce: text,
  ciphertext: text,
});

function bytes(value: string, length?: number): ArrayBuffer {
  const decoded = fromBase64(value);
  if (
    toBase64(decoded) !== value ||
    (length !== undefined && decoded.byteLength !== length)
  )
    throw invalid();
  return decoded;
}

function aad(salt: string, version: number): Uint8Array<ArrayBuffer> {
  return new TextEncoder().encode(
    JSON.stringify([
      FORMAT,
      version,
      "PBKDF2",
      "SHA-512",
      PBKDF2_ITERATIONS,
      salt,
      "AES-256-GCM",
    ]),
  );
}

async function validatePayload(
  key: CryptoKey,
  input: unknown,
): Promise<BackupPayload> {
  const payload = v.parse(payloadSchema, input);
  if (payload.itemFormat === 2 && !Object.hasOwn(payload, "transitionBackup"))
    throw invalid();
  bytes(payload.meta.salt, 16);
  bytes(payload.meta.iv, 12);
  await verifyAuthentication(key, payload.meta, payload.authentication);
  await decryptLockSettings(
    key,
    payload.meta.id,
    payload.lockSettings ?? null,
  );
  if (payload.transitionBackup != null) {
    if (payload.itemFormat !== 2) throw invalid();
    await validateTransitionBackup(key, payload, payload.transitionBackup);
  }
  if (payload.folders !== undefined && payload.itemFormat !== 2)
    throw invalid();
  const folders: Folder[] = [];
  for (const record of payload.folders ?? []) {
    bytes(record.nonce, 12);
    bytes(record.ciphertext);
    const folder = v.parse(
      FolderSchema,
      JSON.parse(await decrypt(key, record.ciphertext, record.nonce)),
    );
    if (folder.id !== record.id) throw invalid();
    folders.push(folder);
  }
  const items: Secret[] = [];
  const ids = new Set<string>();
  for (const record of payload.secrets) {
    if (ids.has(record.id)) throw invalid();
    ids.add(record.id);
    bytes(record.nonce, 12);
    bytes(record.ciphertext);
    const plaintext = await decrypt(key, record.ciphertext, record.nonce);
    const logical =
      payload.itemFormat === 2
        ? v.parse(SecretSchema, JSON.parse(plaintext))
        : transitionLogin(JSON.parse(plaintext));
    if (logical.id !== record.id) throw invalid();
    items.push(logical);
  }
  if (payload.folders !== undefined) validateItemSnapshot({ items, folders });
  return payload;
}

export async function validateTransitionBackup(
  key: CryptoKey,
  current: Pick<BackupPayload, "meta" | "authentication">,
  backup: string,
): Promise<void> {
  try {
    if (new TextEncoder().encode(backup).byteLength > BACKUP_MAX_BYTES)
      throw invalid();
    const envelope = v.parse(envelopeSchema, JSON.parse(backup));
    if (envelope.version !== 1 || envelope.kdf.salt !== current.meta.salt)
      throw invalid();
    bytes(envelope.nonce, 12);
    bytes(envelope.ciphertext);
    const source = JSON.parse(
      await decrypt(
        key,
        envelope.ciphertext,
        envelope.nonce,
        aad(envelope.kdf.salt, 1),
      ),
    );
    if (
      source.itemFormat !== undefined ||
      source.transitionBackup !== undefined ||
      source.folders !== undefined
    )
      throw invalid();
    const recovery = await validatePayload(key, source);
    if (
      JSON.stringify(recovery.meta) !== JSON.stringify(current.meta) ||
      recovery.authentication !== current.authentication
    )
      throw invalid();
  } catch {
    throw new Error(
      "Cannot validate the pre-transition backup. Keep the original source database and use a verified external backup. Do not clear browser data.",
    );
  }
}

export async function encodeBackup(
  key: CryptoKey,
  input: BackupPayload,
): Promise<string> {
  try {
    const payload = await validatePayload(key, input);
    const version =
      payload.folders !== undefined
        ? 3
        : payload.itemFormat === 2
          ? 2
          : 1;
    const encrypted = await encrypt(
      key,
      JSON.stringify(payload),
      aad(payload.meta.salt, version),
    );
    const backup = JSON.stringify({
      format: FORMAT,
      version,
      kdf: {
        name: "PBKDF2",
        hash: "SHA-512",
        iterations: PBKDF2_ITERATIONS,
        salt: payload.meta.salt,
      },
      cipher: "AES-256-GCM",
      ...encrypted,
    });
    if (new TextEncoder().encode(backup).byteLength > BACKUP_MAX_BYTES)
      throw invalid();
    return backup;
  } catch {
    throw new Error(
      "Cannot create a complete encrypted backup. Check the vault records and the 16 MiB / 10,000 record limit. No source data was changed.",
    );
  }
}

export interface BackupRestore {
  readonly id: string;
  readonly name: string;
  readonly recordCount: number;
  restore(): Promise<VaultIdentity>;
}

export async function prepareBackupRestore(
  password: string,
  backup: string,
): Promise<BackupRestore> {
  if (new TextEncoder().encode(backup).byteLength > BACKUP_MAX_BYTES)
    throw invalid();
  let key: CryptoKey;
  let payload: BackupPayload;
  try {
    const envelope = v.parse(envelopeSchema, JSON.parse(backup));
    const salt = bytes(envelope.kdf.salt, 16);
    bytes(envelope.nonce, 12);
    bytes(envelope.ciphertext);
    key = await deriveKey(password, salt);
    const plaintext = await decrypt(
      key,
      envelope.ciphertext,
      envelope.nonce,
      aad(envelope.kdf.salt, envelope.version),
    );
    payload = await validatePayload(key, JSON.parse(plaintext));
    if (
      payload.meta.salt !== envelope.kdf.salt ||
      envelope.version >= 2 !== (payload.itemFormat === 2) ||
      (envelope.version === 3) !== (payload.folders !== undefined)
    )
      throw invalid();
    if (envelope.version === 1 && payload.transitionBackup !== undefined)
      throw invalid();
  } catch {
    throw new Error(
      "Wrong password or invalid encrypted backup. Check the password, complete file, supported version, and 16 MiB / 10,000 record limit. Nothing was restored.",
    );
  }
  let busy = false;
  return Object.freeze({
    id: payload.meta.id,
    name: payload.name,
    recordCount: payload.secrets.length,
    async restore() {
      if (busy)
        throw new Error(
          "Restore is already running. Check the selector before retrying.",
        );
      busy = true;
      try {
        const client = await SharedDb.getInstance().getClient();
        const registry = createVaultRegistry(client);
        if (
          (await registry.listVaultIdentities()).some(
            (entry) => entry.id === payload.meta.id,
          )
        ) {
          throw new Error(
            "This vault ID already exists. Nothing was overwritten. Select the existing vault, or restore in a clean browser profile. Do not delete the source vault.",
          );
        }
        const identity: VaultIdentity = {
          id: payload.meta.id,
          name: payload.name,
          migrated: true,
          status: "ready",
        };
        await stageVault(identity.id, key, payload);
        return identity;
      } finally {
        busy = false;
      }
    },
  });
}

async function verifySharedDb(
  vaultId: string,
  key: CryptoKey,
  payload: BackupPayload,
  client: Client,
): Promise<void> {
  // Verify vault metadata
  const vaultResults = await client.execute(
    `SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults WHERE id = ?`,
    (row: Row) => parseVaultRow(row),
    [dbParam(vaultId)],
  );
  const vault = vaultResults[0];
  if (!vault) throw invalid();

  const expectedVault = [
    payload.meta.id,
    payload.name,
    payload.meta.salt,
    payload.meta.iv,
    payload.meta.createdAt,
    payload.authentication,
  ];
  const actualVault = [
    vault.id,
    vault.name,
    vault.salt,
    vault.iv,
    vault.createdAt,
    vault.authentication ?? "",
  ];
  if (JSON.stringify(actualVault) !== JSON.stringify(expectedVault))
    throw invalid();

  // Verify secrets
  const secretsResults = await client.execute(
    `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
    (row: Row) => parseSecretRow(row),
    [dbParam(vaultId)],
  );
  const expectedRecords = [...payload.secrets]
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .map((record) => ({
      id: record.id,
      vaultId,
      ciphertext: record.ciphertext,
      nonce: record.nonce,
      createdAt: record.createdAt,
      updatedAt: record.updatedAt,
    }));
  const actualSecrets = secretsResults.map((s) => ({
    id: s.id,
    vaultId: s.vaultId,
    ciphertext: s.ciphertext,
    nonce: s.nonce,
    createdAt: s.createdAt,
    updatedAt: s.updatedAt,
  }));
  if (JSON.stringify(actualSecrets) !== JSON.stringify(expectedRecords))
    throw invalid();

  // Verify folders
  const foldersResults = await client.execute(
    `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
    (row: Row) => parseFolderRow(row),
    [dbParam(vaultId)],
  );
  const expectedFolders = [...(payload.folders ?? [])]
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .map((record) => ({
      id: record.id,
      vaultId,
      ciphertext: record.ciphertext,
      nonce: record.nonce,
      createdAt: record.createdAt,
      updatedAt: record.updatedAt,
    }));
  const actualFolders = foldersResults.map((f) => ({
    id: f.id,
    vaultId: f.vaultId,
    ciphertext: f.ciphertext,
    nonce: f.nonce,
    createdAt: f.createdAt,
    updatedAt: f.updatedAt,
  }));
  if (JSON.stringify(actualFolders) !== JSON.stringify(expectedFolders))
    throw invalid();

  // Verify lock settings
  const lockResults = await client.execute(
    `SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
    (row: Row) => parseLockSettingsRow(row),
    [dbParam(vaultId)],
  );
  if (lockResults[0]) {
    const lock = lockResults[0];
    const expectedLock = JSON.parse(payload.lockSettings ?? "{}");
    if (lock.ciphertext !== expectedLock.ciphertext || lock.nonce !== expectedLock.nonce)
      throw invalid();
  }

  await decryptLockSettings(key, payload.meta.id, payload.lockSettings ?? null);
  await verifyAuthentication(key, payload.meta, payload.authentication);
}

export async function stageVault(
  vaultId: string,
  key: CryptoKey,
  payload: BackupPayload,
): Promise<void> {
  try {
    const client = await SharedDb.getInstance().getClient();

    // Check for duplicate vault ID before starting transaction
    const existing = await client.execute(
      `SELECT id FROM vaults WHERE id = ?`,
      () => 1,
      [dbParam(vaultId)],
    );
    if (existing.length > 0) {
      throw new Error(
        "This vault ID already exists. Nothing was overwritten. Select the existing vault, or restore in a clean browser profile. Do not delete the source vault.",
      );
    }

    const tx = await client.transaction();
    try {
      // Insert vault metadata
      await tx.executeRaw(
        `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
        dbParams(
          payload.meta.id,
          payload.name,
          payload.meta.salt,
          payload.meta.iv,
          payload.meta.createdAt,
          payload.authentication,
        ),
      );

      // Insert secrets
      for (const record of payload.secrets) {
        await tx.executeRaw(
          `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
          dbParams(
            record.id,
            vaultId,
            record.ciphertext,
            record.nonce,
            record.createdAt,
            record.updatedAt,
          ),
        );
      }

      // Insert folders
      for (const record of payload.folders ?? []) {
        await tx.executeRaw(
          `INSERT INTO folders (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
          dbParams(
            record.id,
            vaultId,
            record.ciphertext,
            record.nonce,
            record.createdAt,
            record.updatedAt,
          ),
        );
      }

      // Insert lock settings
      if (payload.lockSettings) {
        const lock = JSON.parse(payload.lockSettings);
        await tx.executeRaw(
          `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)`,
          dbParams(
            vaultId,
            lock.ciphertext,
            lock.nonce,
            new Date().toISOString(),
          ),
        );
      }

      await tx.commit();
    } catch (e) {
      await tx.rollback();
      throw e;
    }

    // Verify
    await verifySharedDb(vaultId, key, payload, client);
  } catch {
    throw new Error(
      "Restore storage write could not be verified. No destination was published. Check browser storage and free space, then retry with the backup. Do not clear source data.",
    );
  }
}
