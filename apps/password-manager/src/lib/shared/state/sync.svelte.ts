import type { SyncSDK } from "@ofsdk/sync-sdk";
import {
  BrowserSyncSDK,
  createSqlResourceSync,
  type SqlResourceSync,
} from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import { hasLegacyLocalVaults, openSharedDatabase } from "$lib/vault/storage/migrate-to-shared";
import { StorageClient } from "../../../../../../packages/storage-client/src/index";

let sdk: SyncSDK | undefined;

type Session = NonNullable<typeof authState.session>;

type InitialSync = { controller: AbortController; sync?: SqlResourceSync };
let initialSync: InitialSync | undefined;

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

const creationKey = "password-manager-local-v1";
const resourceName = "Password Manager local database";

function restoreNamespace(session: Session) {
  return JSON.stringify([
    session.issuer,
    session.storage,
    session.applicationUri,
    session.user.sub,
  ]);
}

async function restoredResource(session: Session): Promise<string | undefined> {
  const database = await openSharedDatabase();
  try {
    const tag = await database.initialRestoreTag();
    if (!tag) return undefined;
    const [namespace, id] = JSON.parse(new TextDecoder().decode(tag));
    return namespace === restoreNamespace(session) && typeof id === "string" ? id : undefined;
  } finally {
    database.close();
  }
}

async function findRestoreResource(expected: Session) {
  const resource = await storageAccess(expected).client.findSqlDatabaseByCreationKey(creationKey);
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  return resource?.id;
}

async function uploadLocalDatabase(expected: Session, onResource: (id: string) => void) {
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  if (initialSync) throw new Error("An initial sync is already in progress");
  const pending: InitialSync = { controller: new AbortController() };
  initialSync = pending;
  const { signal } = pending.controller;
  function requireSession() {
    signal.throwIfAborted();
    if (authState.session !== expected) throw new Error("Sign-in session ended");
  }
  let database: Awaited<ReturnType<typeof openSharedDatabase>> | undefined;
  try {
    const { client } = storageAccess(expected);
    database = await openSharedDatabase();
    // Once history exists, a concurrent restore cannot publish a new binding.
    if (await database.initialRestoreIsEligible())
      throw new Error("There is no local database to upload.");
    if (await database.initialRestoreTag())
      throw new Error("This database was restored from Storage. First upload is not available.");
    requireSession();
    // Keep this key and name distinct from the legacy synced-vault Resource.
    const resource = await client.ensureDatabase(
      {
        creationKey,
        name: resourceName,
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
    initialSync = undefined;
  }
}

async function restoreRemoteDatabase(expected: Session, resourceId: string) {
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  if (initialSync) throw new Error("An initial sync is already in progress");
  const pending: InitialSync = { controller: new AbortController() };
  initialSync = pending;
  const { signal } = pending.controller;
  function requireSession() {
    signal.throwIfAborted();
    if (authState.session !== expected) throw new Error("Sign-in session ended");
  }
  let database: Awaited<ReturnType<typeof openSharedDatabase>> | undefined;
  try {
    const { client } = storageAccess(expected);
    database = await openSharedDatabase();
    if (!(await database.initialRestoreIsEligible()) || (await hasLegacyLocalVaults())) {
      throw new Error("Merge required: this browser has local history. No data was changed.");
    }
    requireSession();
    pending.sync = createSqlResourceSync(
      database,
      {
        storeId: resourceId,
        replicaId: crypto.randomUUID(),
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      },
      {
        mode: "first-restore",
        localTag: new TextEncoder().encode(
          JSON.stringify([restoreNamespace(expected), resourceId])
        ),
        storageApiUrl: expected.storage,
        async tokenProvider() {
          requireSession();
          const token = await client.getStorageToken("storage:sync", ["read"], signal);
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
    initialSync = undefined;
  }
}

async function closeSync(): Promise<void> {
  const previous = sdk;
  sdk = undefined;
  initialSync?.controller.abort();
  await Promise.all([previous?.close(), initialSync?.sync?.close()]);
}

export const syncStateModule = {
  supportsReplication,
  closeSync,
  findRestoreResource,
  restoredResource,
  restoreRemoteDatabase,
  uploadLocalDatabase,
  async openDatabase(id: string) {
    return getSync().sql.open(id);
  },
  async listDatabases() {
    return storageAccess().client.listSqlDatabases();
  },
};
