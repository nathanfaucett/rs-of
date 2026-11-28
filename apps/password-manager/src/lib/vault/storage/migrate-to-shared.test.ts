import { Database } from "@ofsdk/sql";
import { afterEach, assert, expect, it } from "vitest";
import { legacyPassword, seedLegacyLocal } from "../../../test/legacy-local-fixture";
import { clearStorage, getStorageClient } from "../../../test/utils";
import { prepareBackupRestore } from "../backup";
import { listVaults, unlockVault } from "../vault";
import { decrypt, deriveKey, fromBase64 } from "./crypto";
import {
  hasLegacyLocalVaults,
  listLegacyLocalVaults,
  migrateLocalVault,
  openSharedDatabase,
  unlockLocalRecovery,
} from "./migrate-to-shared";
import { openLegacyReplicaFence, retiredRevision } from "./retire-legacy-replica";
import { dbParams, parseText } from "./schema";

const revision = async (name: string) => {
  const fence = await openLegacyReplicaFence(name);
  assert(fence);
  fence.close();
  return fence.revision;
};

const update = async (name: string, sql: string, ...params: string[]) => {
  const db = await Database.open(name);
  const client = db.client();
  try {
    await client.executeRaw(sql, dbParams(...params));
  } finally {
    client.close();
    db.close();
  }
};

afterEach(clearStorage);

it.each(["new", "batch", "transition"] as const)(
  "recovers and migrates the pre-unification %s vault without changing its state",
  async (mode) => {
    const source = await seedLegacyLocal(mode);
    const { id, name, databaseName, original, meta, authentication, record, folderRecord } = source;
    const before = await revision(databaseName);
    const previous = await revision(original);
    expect(await listLegacyLocalVaults()).toEqual([expect.objectContaining({ id, name })]);
    const items = {
      items: [expect.objectContaining(source.item)],
      folders: folderRecord ? [source.folder] : [],
    };

    const recovery = await unlockLocalRecovery(id, legacyPassword);
    try {
      expect(recovery.id).toBe(id);
      expect(await recovery.readItems()).toEqual(items);
      expect(await recovery.loadLockSettings()).toEqual(source.settings);
      const backup = await recovery.exportBackup();
      expect(await prepareBackupRestore(legacyPassword, backup)).toMatchObject({
        id,
        name,
        recordCount: 1,
      });
      if (source.backup) {
        // The recovery export retains the validated pre-transition backup.
        const { format, version, kdf, cipher, ciphertext, nonce } = JSON.parse(backup);
        const key = await deriveKey(legacyPassword, fromBase64(meta.salt));
        const aad = new TextEncoder().encode(
          JSON.stringify([format, version, kdf.name, kdf.hash, kdf.iterations, kdf.salt, cipher])
        );
        expect(JSON.parse(await decrypt(key, ciphertext, nonce, aad)).transitionBackup).toBe(
          source.backup
        );
      }
      await expect(recovery.saveLockSettings({ timeoutMinutes: 5 })).rejects.toThrow("read-only");
    } finally {
      await recovery.close();
    }
    expect(await revision(databaseName)).toBe(before);
    expect(await hasLegacyLocalVaults()).toBe(true);

    await migrateLocalVault(id, legacyPassword);
    const vault = await unlockVault(legacyPassword, id);
    try {
      expect(await vault.readItems()).toEqual(items);
      expect(await vault.loadLockSettings()).toEqual(source.settings);
    } finally {
      await vault.close();
    }
    expect(await revision(databaseName)).toBe(retiredRevision);
    if (original !== databaseName) expect(await revision(original)).toBe(previous);
    if (source.backup) {
      const retained = await Database.open(databaseName);
      const client = retained.client();
      try {
        expect(
          await client.execute("SELECT transition_backup FROM vault_meta", (row) =>
            parseText(row, "transition_backup")
          )
        ).toEqual([source.backup]);
      } finally {
        client.close();
        retained.close();
      }
    }
    expect(await hasLegacyLocalVaults()).toBe(false);
    expect(await listVaults()).toEqual([expect.objectContaining({ id, name })]);

    const { client, close } = await getStorageClient();
    try {
      const read = (fields: string, table: string, where = "vault_id") =>
        client.execute(
          `SELECT ${fields} FROM ${table} WHERE ${where} = ?`,
          (row) => fields.split(", ").map((field) => parseText(row, field)),
          dbParams(id)
        );
      expect(await read("id, name, salt, iv, created_at, authentication", "vaults", "id")).toEqual([
        [id, name, meta.salt, meta.iv, meta.createdAt, authentication],
      ]);
      const fields = "id, ciphertext, nonce, created_at, updated_at";
      expect(await read(fields, "secrets")).toEqual([
        [record.id, record.ciphertext, record.nonce, record.createdAt, record.updatedAt],
      ]);
      expect(await read(fields, "folders")).toEqual(
        folderRecord
          ? [
              [
                source.folder.id,
                folderRecord.ciphertext,
                folderRecord.nonce,
                record.createdAt,
                record.updatedAt,
              ],
            ]
          : []
      );
      expect(await read("ciphertext, nonce", "lock_settings")).toEqual([
        [source.lock.ciphertext, source.lock.nonce],
      ]);
    } finally {
      close();
    }
    const reopened = await unlockVault(legacyPassword, id);
    try {
      expect(await reopened.readItems()).toEqual(items);
    } finally {
      await reopened.close();
    }
  },
  20000
);

async function expectRefused(
  source: Awaited<ReturnType<typeof seedLegacyLocal>>,
  message: string,
  password = legacyPassword
) {
  const { id, databaseName } = source;
  const before = await revision(databaseName);
  await expect(unlockLocalRecovery(id, password)).rejects.toThrow(message);
  await expect(migrateLocalVault(id, password)).rejects.toThrow(message);
  expect(await revision(databaseName)).toBe(before);
  expect(await hasLegacyLocalVaults()).toBe(true);
  const shared = await openSharedDatabase();
  try {
    expect(await shared.initialRestoreIsEligible()).toBe(true);
  } finally {
    shared.close();
  }
}

it("rejects the wrong password on both paths without retiring the source", async () => {
  await expectRefused(await seedLegacyLocal("new"), "Wrong password", "not the password");
});

it.each([
  [
    "incomplete migration ledger",
    "DELETE FROM _ofdb_migrations WHERE version = 5",
    "schema or migration history",
  ],
  [
    "missing authentication",
    "UPDATE vault_meta SET authentication = NULL",
    "format or authentication",
  ],
  ["unknown format", "UPDATE vault_meta SET item_format = 3", "format or authentication"],
  [
    "corrupt ciphertext",
    "UPDATE secrets SET ciphertext = 'AA=='",
    "Malformed older vault snapshot",
  ],
  ["missing referenced folder", "DELETE FROM folders", "Malformed older vault snapshot"],
  ["corrupt lock settings", "UPDATE vault_meta SET lock_settings = '{}'", "Invalid"],
])("rejects %s on both paths without retiring the source", async (_name, sql, message) => {
  const source = await seedLegacyLocal("batch");
  await update(source.databaseName, sql);
  await expectRefused(source, message);
});

it("rejects authenticated format-1 rows in a partially transitioned format-2 vault", async () => {
  const source = await seedLegacyLocal("transition");
  await update(
    source.databaseName,
    "UPDATE secrets SET ciphertext = ?, nonce = ?",
    source.oldRecord.ciphertext,
    source.oldRecord.nonce
  );
  await expectRefused(source, "Malformed older vault snapshot");
});

it("rejects a corrupt transition backup without falling back to the previous database", async () => {
  const source = await seedLegacyLocal("transition");
  const previous = await revision(source.original);
  await update(source.databaseName, "UPDATE vault_meta SET transition_backup = '{}' ");
  await expectRefused(source, "Cannot validate the pre-transition backup");
  expect(await revision(source.original)).toBe(previous);
});
