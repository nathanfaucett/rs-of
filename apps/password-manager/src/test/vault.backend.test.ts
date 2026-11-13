import "../../../../e2e/owners.browser";
import { expect, test } from "vitest";
import { createSync } from "@ofsdk/sync-sdk/browser";
import { StorageClient } from "../../../../packages/storage-client/src/index";
import { createSyncVault, unlockSyncVault } from "../lib/vault/sync-vault";
import { MIGRATIONS } from "../lib/vault/storage/schema";
import { backendURL, json, session } from "../../../../e2e/browser-session";

test("encrypted vault CRUD reaches real Storage and rejects a wrong password", async () => {
  const login = await session();
  const user = await json(
    await fetch(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${login.access_token}` },
    })
  );
  const sdk = await createSync({
    apiUrl: `${backendURL}/storage`,
    appId: "local-login",
    userId: user.sub,
    getToken: async () => login.access_token,
  });
  const storage = new StorageClient({
    idpBaseUrl: `${backendURL}/idp`,
    storageBaseUrl: `${backendURL}/storage`,
    clientId: "local-login",
    bearerToken: () => login.access_token,
  });
  const handle = await sdk.sql.create(`vault-e2e-${crypto.randomUUID()}`);
  try {
    for (const migration of MIGRATIONS) await handle.execute(migration.sql);
    const password = "e2e-master-password";
    const vault = await createSyncVault(password, "E2E vault", handle);
    const item = await vault.encryptAndStore("E2E private note");
    expect((await vault.loadSecrets()).map((entry) => entry.id)).toContain(item.id);
    await handle.sync({ direction: "push" });
    const remote = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext FROM secrets", params: [] }] },
      ["read"]
    );
    expect(remote.results[0].rows).toHaveLength(1);
    expect(JSON.stringify(remote)).not.toContain("E2E private note");
    expect(JSON.stringify(remote)).not.toContain(password);
    await expect(unlockSyncVault("wrong-password", vault.id, handle)).rejects.toThrow();
    const reopened = await unlockSyncVault(password, vault.id, handle);
    expect(reopened).not.toBeNull();
    expect(await reopened!.decryptSecret(item)).toBe("E2E private note");
    await reopened!.encryptAndStore(JSON.stringify({ ...item, notes: "Updated private note" }));
    expect((await reopened!.loadSecrets())[0].notes).toBe("Updated private note");
    await handle.sync({ direction: "push" });
    const updated = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext FROM secrets", params: [] }] },
      ["read"]
    );
    expect(updated.results[0].rows).toHaveLength(1);
    expect(updated.results[0].rows).not.toEqual(remote.results[0].rows);
    expect(JSON.stringify(updated)).not.toContain("Updated private note");
    await reopened!.deleteSecret(item.id);
    await handle.sync({ direction: "push" });
    expect(await reopened!.loadSecrets()).toEqual([]);
    const deleted = await storage.executeSql(
      handle.id,
      { statements: [{ sql: "SELECT ciphertext FROM secrets", params: [] }] },
      ["read"]
    );
    expect(deleted.results[0].rows).toEqual([]);
  } finally {
    await sdk.close();
    await storage.deleteDatabase(handle.id);
  }
});
