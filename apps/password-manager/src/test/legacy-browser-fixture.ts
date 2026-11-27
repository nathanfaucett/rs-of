import { BrowserSyncSDK } from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import { createSyncVault } from "./legacy-sync-vault";

export async function createLegacyVault(resourceId: string, password: string, name: string) {
  const session = authState.session;
  if (!session) throw new Error("Sign in before creating the older replica fixture.");
  const sdk = new BrowserSyncSDK({
    apiUrl: session.storage,
    appId: session.clientId,
    userId: session.user.sub,
    getToken: () => authState.getAccessToken(session),
    getStorageToken: async () => {
      throw new Error("The SDK replaces this provider.");
    },
  });
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
