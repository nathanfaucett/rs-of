import { createSync, type SyncSDK, type SyncConfig, type DatabaseHandle, type SyncState, type ChangeEvent, type SyncSession } from "@ofsdk/sync-sdk";
import { authState } from "$lib/auth/auth.svelte";
import { env } from "$env/dynamic/public";

const STORAGE_SERVICE_URL = env.PUBLIC_STORAGE_URL ?? "https://storage.localhost:1355";
const APP_ID = "password-manager";

let syncSDK = $state<SyncSDK | null>(null);
let isInitializing = $state(false);
let initError = $state<string | null>(null);

function getUserId(): string | null {
  return authState.user?.sub ?? null;
}

async function initializeSync(): Promise<void> {
  if (syncSDK || isInitializing) return;

  const userId = getUserId();
  if (!userId) {
    initError = "No authenticated user";
    return;
  }

  isInitializing = true;
  initError = null;

  try {
    const config: SyncConfig = {
      apiUrl: STORAGE_SERVICE_URL,
      appId: APP_ID,
      userId,
      getToken: () => authState.getAccessToken(),
    };

    syncSDK = await createSync(config);
  } catch (e) {
    initError = e instanceof Error ? e.message : "Failed to initialize sync";
    syncSDK = null;
  } finally {
    isInitializing = false;
  }
}

async function closeSync(): Promise<void> {
  if (syncSDK) {
    await syncSDK.close();
    syncSDK = null;
  }
}

function getSyncSDK(): SyncSDK | null {
  return syncSDK;
}

async function createDatabase(name: string): Promise<DatabaseHandle | null> {
  if (!syncSDK) return null;
  return syncSDK.sql.create(name);
}

async function openDatabase(databaseId: string): Promise<DatabaseHandle | null> {
  if (!syncSDK) return null;
  return syncSDK.sql.open(databaseId);
}

async function listDatabases(): Promise<{ id: string; name: string | null }[]> {
  if (!syncSDK) return [];
  return syncSDK.sql.list();
}

async function syncToDevice(resourceId: string, targetDeviceId: string): Promise<SyncSession | null> {
  if (!syncSDK) return null;
  return syncSDK.syncToDevice(resourceId, targetDeviceId);
}

export const syncStateModule = {
  get sdk() {
    return syncSDK;
  },
  get isInitializing() {
    return isInitializing;
  },
  get initError() {
    return initError;
  },
  initializeSync,
  closeSync,
  getSyncSDK,
  createDatabase,
  openDatabase,
  listDatabases,
  syncToDevice,
};

$effect(() => {
  if (authState.isAuthenticated) {
    void initializeSync();
  } else {
    void closeSync();
  }
});
