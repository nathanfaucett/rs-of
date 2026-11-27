import type { SyncSDK } from "@ofsdk/sync-sdk";
import {
  BrowserSyncSDK,
  createSqlResourceSync,
  type SqlResourceSync,
} from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import { openSharedDatabase } from "$lib/vault/storage/migrate-to-shared";
import { StorageClient } from "../../../../../../packages/storage-client/src/index";

let sdk: SyncSDK | undefined;

type Session = NonNullable<typeof authState.session>;

type FirstUpload = { controller: AbortController; sync?: SqlResourceSync };
let firstUpload: FirstUpload | undefined;

function storageAccess(session = authState.session) {
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

async function uploadLocalDatabase(expected: Session, onResource: (id: string) => void) {
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  if (firstUpload) throw new Error("A first upload is already in progress");
  const pending: FirstUpload = { controller: new AbortController() };
  firstUpload = pending;
  const { signal } = pending.controller;
  function requireSession() {
    signal.throwIfAborted();
    if (authState.session !== expected) throw new Error("Sign-in session ended");
  }
  let database: Awaited<ReturnType<typeof openSharedDatabase>> | undefined;
  try {
    const { client } = storageAccess(expected);
    database = await openSharedDatabase();
    requireSession();
    // Keep this key and name distinct from the legacy synced-vault Resource.
    const resource = await client.ensureDatabase(
      {
        creationKey: "password-manager-local-v1",
        name: "Password Manager local database",
      },
      signal
    );
    requireSession();
    onResource(resource.id);
    pending.sync = createSqlResourceSync(
      database,
      {
        storeId: resource.id,
        replicaId: crypto.randomUUID(),
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      },
      {
        mode: "first-upload",
        storageApiUrl: expected.storage,
        async tokenProvider() {
          requireSession();
          const token = await client.getStorageToken("storage:sync", ["read", "write"], signal);
          requireSession();
          return token;
        },
      }
    );
    await pending.sync.sync();
    requireSession();
  } finally {
    await pending.sync?.close();
    database?.close();
    firstUpload = undefined;
  }
}

async function closeSync(): Promise<void> {
  const previous = sdk;
  sdk = undefined;
  firstUpload?.controller.abort();
  await Promise.all([previous?.close(), firstUpload?.sync?.close()]);
}

export const syncStateModule = {
  supportsReplication,
  closeSync,
  uploadLocalDatabase,
  async openDatabase(id: string) {
    return getSync().sql.open(id);
  },
  async listDatabases() {
    return storageAccess().client.listSqlDatabases();
  },
};
