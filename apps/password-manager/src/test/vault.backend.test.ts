import "../../../../e2e/owners.browser";
import { createSync } from "@ofsdk/sync-sdk/browser";
import { expect, test } from "vitest";
import { backendURL, json, session } from "../../../../e2e/browser-session";
import { StorageClient } from "../../../../packages/storage-client/src/index";
import { MIGRATIONS } from "../lib/vault/storage/schema";
import { createSyncVault, unlockSyncVault } from "../lib/vault/sync-vault";

test("encrypted vault CRUD reaches real Storage and reopens legacy SQL timestamps", async () => {
  const login = await session();
  const user: { sub: string } = await json(
    await fetch(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${login.access_token}` },
    })
  );
  const storage = new StorageClient({
    idpBaseUrl: `${backendURL}/idp`,
    storageBaseUrl: `${backendURL}/storage`,
    clientId: "local-login",
    bearerToken: () => login.access_token,
  });
  const sdk = await createSync({
    apiUrl: `${backendURL}/storage`,
    appId: "local-login",
    userId: user.sub,
    getToken: async () => login.access_token,
    getStorageToken: (resource, actions) => storage.getStorageToken(resource, actions),
  });
  let handle = await sdk.sql.create(`vault-e2e-${crypto.randomUUID()}`);
  const resourceId = handle.id;
  try {
    for (const migration of MIGRATIONS) await handle.execute(migration.sql);
    const password = "e2e-master-password";
    const vault = await createSyncVault(password, "E2E vault", handle);
    const now = new Date().toISOString();
    const plaintext = JSON.stringify({
      id: crypto.randomUUID(),
      kind: "secureNote",
      name: "Test note",
      notes: "E2E private note",
      createdAt: now,
      updatedAt: now,
    });
    const item = await vault.encryptAndStore(plaintext);
    expect((await vault.loadSecrets()).map((entry) => entry.id)).toContain(item.id);
    const folder = await vault.createFolder("Private notes");
    await vault.saveLockSettings({ timeoutMinutes: 1 });
    // Legacy SQL permitted timestamp text that current writes do not produce.
    const legacyTimestamp = "2026-11-26 12:00:00";
    for (const table of ["secrets", "folders"]) {
      await handle.execute(`UPDATE ${table} SET created_at = ?, updated_at = ?`, [
        legacyTimestamp,
        legacyTimestamp,
      ]);
    }
    await handle.execute("UPDATE lock_settings SET updated_at = ?", [legacyTimestamp]);
    await handle.sync({ direction: "push" });
    const remote = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }] },
      ["read"]
    );
    expect(remote.results[0]?.rows).toEqual([
      [
        { type: "text", value: expect.stringMatching(/^[A-Za-z0-9+/]+=*$/) },
        { type: "text", value: expect.stringMatching(/^[A-Za-z0-9+/]{16}$/) },
      ],
    ]);
    expect(JSON.stringify(remote)).not.toContain("E2E private note");
    expect(JSON.stringify(remote)).not.toContain(password);
    await handle.sync({ direction: "push" });
    const repeated = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }] },
      ["read"]
    );
    expect(repeated.results[0]?.rows).toEqual(remote.results[0]?.rows);
    expect(JSON.stringify(repeated)).not.toContain("E2E private note");
    expect(JSON.stringify(repeated)).not.toContain(password);
    await vault.close();
    handle = await sdk.sql.open(resourceId);
    expect(handle.id).toBe(resourceId);
    await expect(unlockSyncVault("wrong-password", vault.id, handle)).rejects.toThrow();
    const reopened = await unlockSyncVault(password, vault.id, handle);
    expect(reopened).not.toBeNull();
    if (!reopened) throw new Error("Expected vault to reopen");
    expect(reopened.id).toBe(vault.id);
    expect(await reopened.loadLockSettings()).toEqual({ timeoutMinutes: 1 });
    expect(await reopened.readItems()).toEqual({ folders: [folder], items: [item] });
    expect(await reopened.decryptSecret(item)).toBe(plaintext);
    const edited = { ...item, folderId: folder.id, notes: "Updated private note" };
    await reopened.mutateItems({ upsert: [edited] });
    expect(await reopened.readItems()).toEqual({ folders: [folder], items: [edited] });
    await handle.sync({ direction: "push" });
    const updated = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }] },
      ["read"]
    );
    expect(updated.results[0]?.rows).toHaveLength(1);
    expect(updated.results[0]?.rows).not.toEqual(remote.results[0]?.rows);
    expect(JSON.stringify(updated)).not.toContain("Updated private note");
    expect(JSON.stringify(updated)).not.toContain(password);
    await handle.sync({ direction: "push" });
    const repeatedUpdate = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }] },
      ["read"]
    );
    expect(repeatedUpdate.results[0]?.rows).toEqual(updated.results[0]?.rows);
    expect(JSON.stringify(repeatedUpdate)).not.toContain("Updated private note");
    expect(JSON.stringify(repeatedUpdate)).not.toContain(password);
    await reopened.deleteSecret(item.id);
    await handle.sync({ direction: "push" });
    expect(await reopened.loadSecrets()).toEqual([]);
    const deleted = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }] },
      ["read"]
    );
    expect(deleted.results[0]?.rows).toEqual([]);
    await handle.sync({ direction: "push" });
    const repeatedDelete = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }] },
      ["read"]
    );
    expect(repeatedDelete.results[0]?.rows).toEqual([]);
    expect(JSON.stringify(repeatedDelete)).not.toContain(password);
  } finally {
    await sdk.close();
    await storage.deleteDatabase(resourceId);
  }
});
