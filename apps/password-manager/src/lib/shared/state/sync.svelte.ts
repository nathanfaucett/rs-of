import type { DatabaseHandle, SyncConfig, SyncSDK } from "@ofsdk/sync-sdk";
import { createSync } from "@ofsdk/sync-sdk/browser";
import { env } from "$env/dynamic/public";
import { authState } from "$lib/auth/auth.svelte";
import { StorageClient } from "../../../../../../packages/storage-client/src/index";

const { PUBLIC_STORAGE_URL } = env;
const STORAGE_SERVICE_URL = PUBLIC_STORAGE_URL ?? "https://storage.localhost:1355";
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
    const getToken = () => authState.getAccessToken();
    const storageClient = new StorageClient({
      idpBaseUrl: STORAGE_SERVICE_URL.replace("/storage", "/idp"),
      storageBaseUrl: STORAGE_SERVICE_URL,
      clientId: APP_ID,
      bearerToken: getToken,
    });
    const config: SyncConfig = {
      apiUrl: STORAGE_SERVICE_URL,
      appId: APP_ID,
      userId,
      getToken,
      getStorageToken: (resource, actions) => storageClient.getStorageToken(resource, actions),
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
  createDatabase,
  openDatabase,
  listDatabases,
};
