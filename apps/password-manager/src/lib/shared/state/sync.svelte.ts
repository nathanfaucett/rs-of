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

type Exchange = { controller: AbortController; sync?: SqlResourceSync };
let active: Exchange | undefined;
const bindingLock = "password-manager-v1:storage-binding";

type Binding = { namespace: string; id: string; detached: boolean };
type Database = Awaited<ReturnType<typeof openSharedDatabase>>;

function storageAccess(session = authState.session) {
  if (!session || session !== authState.session) throw new Error("Sign in to sync first");
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

function getSync(expected: Session) {
  const { session, client, getToken } = storageAccess(expected);
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

function namespace(session: Session) {
  return JSON.stringify([
    session.issuer,
    session.storage,
    session.applicationUri,
    session.user.sub,
  ]);
}

function encodeBinding(binding: Binding) {
  return new TextEncoder().encode(
    JSON.stringify([binding.namespace, binding.id, binding.detached])
  );
}

async function readBinding(database: Database) {
  const tag = await database.initialRestoreTag();
  if (!tag) return undefined;
  const value: unknown = JSON.parse(new TextDecoder().decode(tag));
  // Ticket 03 stored the same tuple without the detachment flag.
  if (
    !Array.isArray(value) ||
    typeof value[0] !== "string" ||
    typeof value[1] !== "string" ||
    !value[1] ||
    (value.length !== 2 && (value.length !== 3 || typeof value[2] !== "boolean"))
  )
    throw new Error("Invalid local Storage binding. Your vaults are still local.");
  const binding: Binding = { namespace: value[0], id: value[1], detached: value[2] === true };
  return { binding, tag };
}

async function getBinding(expected: Session) {
  const database = await openSharedDatabase();
  try {
    const record = await readBinding(database);
    if (!record) return { status: "unbound" } as const;
    if (record.binding.namespace !== namespace(expected))
      return { status: "different-account" } as const;
    return { status: "bound", id: record.binding.id, detached: record.binding.detached } as const;
  } finally {
    database.close();
  }
}

async function findRestoreResource(expected: Session, signal?: AbortSignal) {
  const resource = await storageAccess(expected).client.findSqlDatabaseByCreationKey(
    creationKey,
    signal
  );
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  return resource?.id;
}

export class BindingSaveError extends Error {}

async function savePublishedBinding(database: Database, tag: Uint8Array) {
  try {
    if (await database.replaceInitialRestoreTag(undefined, tag)) return;
  } catch (cause) {
    throw new BindingSaveError(
      "Storage confirmed publication, but the local binding could not be saved. Your vaults stay local.",
      { cause }
    );
  }
  throw new BindingSaveError(
    "Storage confirmed publication, but a competing local binding was kept."
  );
}

type Intent =
  | { mode: "first-upload"; onResource: (id: string) => void }
  | { mode: "first-restore"; id: string }
  | { mode: "established" };

async function exchange(expected: Session, intent: Intent) {
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  if (active) throw new Error("A sync exchange is already in progress");
  const pending: Exchange = { controller: new AbortController() };
  active = pending;
  const { signal } = pending.controller;
  function requireSession() {
    signal.throwIfAborted();
    if (authState.session !== expected) throw new Error("Sign-in session ended");
  }
  try {
    await navigator.locks.request(bindingLock, { signal }, async () => {
      requireSession();
      const database = await openSharedDatabase();
      try {
        const { client } = storageAccess(expected);
        const record = await readBinding(database);
        const owner = namespace(expected);
        let id: string;
        if (intent.mode === "established") {
          if (!record || record.binding.namespace !== owner || record.binding.detached)
            throw new Error("There is no active Storage binding for this account.");
          id = record.binding.id;
        } else {
          if (record) throw new Error("This database already has a verified Storage binding.");
          const empty = await database.initialRestoreIsEligible();
          if (intent.mode === "first-restore") {
            if (!empty || (await hasLegacyLocalVaults()))
              throw new Error(
                "Merge required: this browser has local history. No data was changed."
              );
            requireSession();
            if ((await findRestoreResource(expected, signal)) !== intent.id)
              throw new Error("The remote Resource changed. Find it again before restoring.");
            id = intent.id;
          } else {
            if (empty) throw new Error("There is no local database to upload.");
            requireSession();
            const resource = await client.ensureDatabase(
              { creationKey, name: resourceName },
              signal
            );
            requireSession();
            id = resource.id;
            intent.onResource(id);
          }
        }
        requireSession();
        const tag = encodeBinding({ namespace: owner, id, detached: false });
        pending.sync = createSqlResourceSync(
          database,
          {
            storeId: id,
            replicaId: crypto.randomUUID(),
            creationEpoch: 0n,
            schemaFingerprint: new Uint8Array(32),
          },
          {
            ...(intent.mode === "first-restore"
              ? { mode: intent.mode, localTag: tag }
              : { mode: intent.mode }),
            storageApiUrl: expected.storage,
            async tokenProvider() {
              requireSession();
              const token = await client.getStorageToken(
                "storage:sync",
                intent.mode === "first-restore" ? ["read"] : ["read", "write"],
                signal
              );
              requireSession();
              return token;
            },
          }
        );
        await pending.sync.sync();
        // Once the receipt is verified, finish the local CAS. Sign-out detaches under this same lock.
        if (intent.mode === "first-upload") await savePublishedBinding(database, tag);
        requireSession();
      } finally {
        await pending.sync?.close();
        database.close();
      }
    });
  } finally {
    active = undefined;
  }
}

function cancelExchange() {
  active?.controller.abort();
  return active?.sync?.close();
}

function watchDetach() {
  const channel = new BroadcastChannel(bindingLock);
  channel.onmessage = () => void cancelExchange();
  return () => channel.close();
}

function stopAcrossTabs() {
  void cancelExchange();
  const channel = new BroadcastChannel(bindingLock);
  channel.postMessage("detach");
  channel.close();
}

async function writeDetached(expected: Session, detached: boolean, allowMissing = false) {
  const database = await openSharedDatabase();
  try {
    const record = await readBinding(database);
    if (!record || record.binding.namespace !== namespace(expected)) {
      if (allowMissing) return;
      throw new Error("There is no Storage binding for this account.");
    }
    if (record.binding.detached === detached) return;
    if (
      !(await database.replaceInitialRestoreTag(
        record.tag,
        encodeBinding({ ...record.binding, detached })
      ))
    )
      throw new Error("The local Storage binding changed. Try again.");
  } finally {
    database.close();
  }
}

async function setDetached(expected: Session, detached: boolean) {
  if (detached) stopAcrossTabs();
  await navigator.locks.request(bindingLock, async () => {
    if (authState.session !== expected || (!detached && !authState.hasVerifiedToken(expected)))
      throw new Error("Sign-in session changed. Try again after sign-in.");
    await writeDetached(expected, detached);
  });
}

async function signOut(expected: Session) {
  stopAcrossTabs();
  await navigator.locks.request(bindingLock, async () => {
    if (authState.session !== expected) return;
    await writeDetached(expected, true, true);
    if (authState.session === expected) await authState.logout();
  });
}

async function closeSync(): Promise<void> {
  const previous = sdk;
  sdk = undefined;
  await Promise.all([previous?.close(), cancelExchange()]);
}

async function openDatabase(id: string, expected = authState.session) {
  if (!expected) throw new Error("Sign in to sync first");
  const owner = getSync(expected);
  const handle = await owner.sql.open(id);
  if (expected !== authState.session || owner !== sdk) {
    await handle.close();
    throw new Error("Sign-in session ended");
  }
  return handle;
}

export const syncStateModule = {
  supportsReplication,
  closeSync,
  findRestoreResource,
  getBinding,
  watchDetach,
  setDetached,
  signOut,
  syncBoundDatabase: (session: Session) => exchange(session, { mode: "established" }),
  restoreRemoteDatabase: (session: Session, id: string) =>
    exchange(session, { mode: "first-restore", id }),
  uploadLocalDatabase: (session: Session, onResource: (id: string) => void) =>
    exchange(session, { mode: "first-upload", onResource }),
  openDatabase,
  async openLegacyDatabase() {
    const { client, session } = storageAccess();
    const databases = await client.listSqlDatabases();
    const matching = databases.filter((entry) => entry.name === "password-manager-main");
    if (matching.length > 1)
      throw new Error("More than one Password Manager Resource exists. No vault data was changed.");
    return matching[0] ? openDatabase(matching[0].id, session) : null;
  },
};
