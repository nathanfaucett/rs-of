import { syncStateModule } from "$lib/shared/state/sync.svelte";
import { createSyncVault } from "$lib/vault/sync-vault";

export async function createLegacyVault(resourceId: string, password: string, name: string) {
  const handle = await syncStateModule.openDatabase(resourceId);
  try {
    await createSyncVault(password, name, handle);
    await handle.sync({ direction: "bidirectional" });
  } finally {
    await handle.close();
  }
}
