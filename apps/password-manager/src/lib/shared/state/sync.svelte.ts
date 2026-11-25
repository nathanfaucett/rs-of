import type { SyncSDK } from "@ofsdk/sync-sdk";
import { BrowserSyncSDK } from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import { StorageClient } from "../../../../../../packages/storage-client/src/index";

let sdk: SyncSDK | undefined;

function storageAccess() {
  const session = authState.session;
  if (!session) throw new Error("Sign in to sync first");
  const getToken = () => authState.getAccessToken(session);
  const client = new StorageClient({
    idpBaseUrl: session.idp,
    storageBaseUrl: session.storage,
    clientId: session.clientId,
    bearerToken: getToken,
    fetch: (input, init) => fetch(input, { ...init, credentials: "omit", redirect: "error" }),
  });
  return { session, client, getToken };
}

function supportsReplication(session = authState.session): boolean {
  // The legacy SDK derives its IdP from the Storage URL. Never use a guessed authority.
  if (!session) return false;
  const { origin, pathname } = new URL(session.storage);
  return (
    pathname.endsWith("/storage") &&
    `${origin}${pathname.replace(/\/storage$/, "/idp")}` === session.idp
  );
}

function getSync() {
  const { session, client, getToken } = storageAccess();
  if (!supportsReplication(session)) {
    throw new Error("Vault replication is not available with separate IdP and Storage bases.");
  }
  sdk ??= new BrowserSyncSDK({
    apiUrl: session.storage,
    appId: session.clientId,
    userId: session.user.sub,
    getToken,
    getStorageToken: (resource, actions) => client.getStorageToken(resource, actions),
  });
  return sdk;
}

async function closeSync(): Promise<void> {
  const previous = sdk;
  sdk = undefined;
  await previous?.close();
}

export const syncStateModule = {
  supportsReplication,
  closeSync,
  async createDatabase(name: string) {
    const legacy = getSync();
    const database = await storageAccess().client.ensureDatabase({ creationKey: name, name });
    return legacy.sql.open(database.id);
  },
  async openDatabase(id: string) {
    return getSync().sql.open(id);
  },
  async listDatabases() {
    return storageAccess().client.listSqlDatabases();
  },
};
