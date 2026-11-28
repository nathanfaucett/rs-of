import { BrowserSyncSDK } from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import { createSyncVault } from "./legacy-sync-vault";

function legacySDK(session: NonNullable<typeof authState.session>) {
  return new BrowserSyncSDK({
    apiUrl: session.storage,
    appId: session.clientId,
    userId: session.user.sub,
    getToken: () => authState.getAccessToken(session),
    getStorageToken: async () => {
      throw new Error("The SDK replaces this provider.");
    },
  });
}

export async function createLegacyVault(resourceId: string, password: string, name: string) {
  const session = authState.session;
  if (!session) throw new Error("Sign in before creating the older replica fixture.");
  const sdk = legacySDK(session);
  try {
    const handle = await sdk.sql.open(resourceId);
    const vault = await createSyncVault(password, name, handle);
    const note = (name: string) =>
      JSON.stringify({
        id: crypto.randomUUID(),
        kind: "secureNote",
        name,
        notes: name,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      });
    const item = await vault.encryptAndStore(note("Legacy note before sync"));
    const removed = await vault.encryptAndStore(note("Legacy note removed in this browser"));
    await vault.saveLockSettings({ timeoutMinutes: 5 });
    await handle.execute("UPDATE lock_settings SET updated_at = ?", ["2026-11-26 12:00:00"]);
    await handle.sync({ direction: "bidirectional" });
    await vault.mutateItems({
      upsert: [{ ...item, name: "Browser-only edit", notes: "Kept encrypted across upgrade" }],
      deleteIds: [removed.id],
    });
    return { vaultId: vault.id, itemId: item.id, deletedId: removed.id };
  } finally {
    await sdk.close();
  }
}

let cachedWriter: Awaited<ReturnType<typeof openCachedWriter>> | undefined;

async function openCachedWriter(id: string) {
  const { Database } = await import("@ofsdk/sql");
  const db = await Database.open(id);
  const client = db.client();
  db.close();
  return client;
}

let staleWrite: Awaited<ReturnType<NonNullable<typeof cachedWriter>["transaction"]>> | undefined;

export async function stageCachedLegacyWrite(id: string) {
  cachedWriter = await openCachedWriter(id);
  staleWrite = await cachedWriter.transaction();
  await staleWrite.executeRaw("DELETE FROM secrets");
}

export async function probeRetiredLegacyWrites(id: string) {
  if (!staleWrite || !cachedWriter) throw new Error("Missing staged legacy writer.");
  const refused = async (write: Promise<unknown>) =>
    write.then(
      () => false,
      () => true
    );
  const fresh = await openCachedWriter(id);
  try {
    return {
      stale: await refused(staleWrite.commit()),
      reopened: await refused(fresh.executeRaw("DELETE FROM secrets")),
    };
  } finally {
    staleWrite.close();
    cachedWriter.close();
    fresh.close();
  }
}

export async function createWrongLegacySource(id: string) {
  const { legacySyncDDL } = await import("./legacy-sync-ddl");
  const session = authState.session;
  if (!session) throw new Error("Sign in before creating the unrelated Resource fixture.");
  const sdk = legacySDK(session);
  try {
    const handle = await sdk.sql.open(id);
    for (const sql of legacySyncDDL) {
      await handle.execute(sql.replace(/^ {2}name TEXT NOT NULL,\n/m, ""));
    }
    const { createAuthentication, deriveKey, fromBase64 } = await import(
      "$lib/vault/storage/crypto"
    );
    const meta = {
      id: crypto.randomUUID(),
      salt: btoa("0".repeat(16)),
      iv: btoa("0".repeat(12)),
      createdAt: "2026-11-26T12:00:00Z",
    };
    const key = await deriveKey("unrelated fixture password", fromBase64(meta.salt));
    await handle.execute(
      "INSERT INTO vaults (id, salt, iv, created_at, authentication) VALUES (?, ?, ?, ?, ?)",
      [meta.id, meta.salt, meta.iv, meta.createdAt, await createAuthentication(key, meta)]
    );
    await handle.sync({ direction: "bidirectional" });
  } finally {
    await sdk.close();
  }
}

export async function wrongLegacySourceState(id: string, write = false) {
  const { Database } = await import("@ofsdk/sql");
  const { openLegacyReplicaFence } = await import("$lib/vault/storage/retire-legacy-replica");
  const db = await Database.open(id);
  const client = db.client();
  try {
    if (write) await client.executeRaw("UPDATE vaults SET migrated = 0");
    const fence = await openLegacyReplicaFence(id);
    fence?.close();
    return {
      revision: fence?.revision,
      history: Array.from(await db.historyRevision()),
    };
  } finally {
    client.close();
    db.close();
  }
}
