import type { SyncSDK } from "@ofsdk/sync-sdk";
import {
  BrowserSyncSDK,
  createSqlResourceSync,
  type SqlResourceSync,
  type SqlResourceSyncStatus,
  startSqlResourceSync,
} from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import { hasLegacyLocalVaults, openSharedDatabase } from "$lib/vault/storage/migrate-to-shared";
import { StorageClient } from "../../../../../../packages/storage-client/src/index";

let sdk: SyncSDK | undefined;

type Session = NonNullable<typeof authState.session>;

type Exchange = { controller: AbortController; sync?: SqlResourceSync };
let active: Exchange | undefined;
const bindingLock = "password-manager-v1:storage-binding";
const bindingTimeout = 2_000;
const receiptKey = `${bindingLock}:receipt`;
let receiptInMemory: Receipt | null | undefined;

type Binding = { namespace: string; id: string; confirmationId: string };
type Receipt = Binding & { version: 1; loginState: string; expected: null };
type Database = Awaited<ReturnType<typeof openSharedDatabase>>;
let attachment = $state.raw<{
  session: Session;
  id: string;
  sync: SqlResourceSync;
  database: Database;
}>();
let syncStatus = $state.raw<SqlResourceSyncStatus>();
let closingAttachment: Promise<void> | undefined;

function getSyncStatus(session: Session | null, id: string | undefined) {
  return attachment?.session === session && attachment?.id === id ? syncStatus : undefined;
}

function resourceIdentity(id: string) {
  return {
    storeId: id,
    replicaId: crypto.randomUUID(),
    creationEpoch: 0n,
    schemaFingerprint: new Uint8Array(32),
  };
}

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
    JSON.stringify([binding.namespace, binding.id, binding.confirmationId])
  );
}

async function readBinding(database: Database) {
  const tag = await database.initialRestoreTag();
  if (!tag) return undefined;
  const revision = new TextDecoder().decode(tag);
  const value: unknown = JSON.parse(revision);
  // Older bindings have no confirmation ID. Keep the destination, but require reattachment.
  if (
    !Array.isArray(value) ||
    typeof value[0] !== "string" ||
    typeof value[1] !== "string" ||
    !value[1] ||
    (value.length !== 2 &&
      (value.length !== 3 ||
        (typeof value[2] !== "boolean" && (typeof value[2] !== "string" || !value[2]))))
  )
    throw new Error("Invalid local Storage binding. Your vaults are still local.");
  return {
    namespace: value[0],
    id: value[1],
    revision: typeof value[2] === "string" ? value[2] : revision,
  };
}

async function getBinding(expected: Session) {
  const owner = namespace(expected);
  const database = await openSharedDatabase();
  try {
    const record = await readBinding(database);
    if (!record) {
      const receipt = readReceipt(expected);
      return receipt
        ? ({ status: "pending-save", id: receipt.id } as const)
        : ({ status: "unbound" } as const);
    }
    if (record.namespace !== owner) return { status: "different-account" } as const;
    return { status: "bound", id: record.id } as const;
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

function readReceipt(expected: Session): Receipt | undefined {
  try {
    const value =
      receiptInMemory === undefined
        ? JSON.parse(localStorage.getItem(receiptKey) ?? "null")
        : receiptInMemory;
    if (
      value?.version === 1 &&
      value.loginState === expected.loginState &&
      value.namespace === namespace(expected) &&
      typeof value.id === "string" &&
      value.id &&
      typeof value.confirmationId === "string" &&
      value.confirmationId &&
      value.expected === null
    )
      return value;
  } catch {
    // Without a readable receipt, remote history cannot authorize a local save.
  }
  return undefined;
}

function retainReceipt(receipt: Receipt) {
  try {
    localStorage.setItem(receiptKey, JSON.stringify(receipt));
    receiptInMemory = undefined;
  } catch {
    receiptInMemory = receipt;
  }
}

function clearLocal(key: string) {
  try {
    localStorage.removeItem(key);
  } catch {
    try {
      localStorage.setItem(key, "");
    } catch {
      return false;
    }
  }
  return true;
}

function clearReceipt() {
  receiptInMemory = clearLocal(receiptKey) ? undefined : null;
}

function clearPendingConfirmation(expected: Session) {
  if (readReceipt(expected)) clearReceipt();
}

async function savePublishedBinding(database: Database, expected: Session) {
  try {
    const record = await readBinding(database);
    const receipt = readReceipt(expected);
    if (!authState.hasVerifiedToken(expected) || !receipt)
      throw new BindingSaveError("There is no verified upload to save for this login session.");
    if (record)
      throw new BindingSaveError(
        "Storage confirmed publication, but a competing local binding was kept."
      );
    if (
      await database.replaceInitialRestoreTag(receipt.expected ?? undefined, encodeBinding(receipt))
    ) {
      if (readReceipt(expected)?.confirmationId === receipt.confirmationId) clearReceipt();
      return;
    }
  } catch (cause) {
    if (cause instanceof BindingSaveError) throw cause;
    throw new BindingSaveError(
      "Storage confirmed publication, but the local binding could not be saved. Retry the local save.",
      { cause }
    );
  }
  throw new BindingSaveError(
    "Storage confirmed publication, but a competing local binding was kept."
  );
}

async function retryBindingSave(expected: Session) {
  const signal = AbortSignal.timeout(bindingTimeout);
  await navigator.locks.request(bindingLock, { signal }, async () => {
    const database = await openSharedDatabase();
    try {
      signal.throwIfAborted();
      await savePublishedBinding(database, expected);
    } finally {
      database.close();
    }
  });
}

type Intent =
  | { mode: "first-upload"; onResource: (id: string) => void }
  | { mode: "first-restore"; id: string };

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
        const confirmationId = crypto.randomUUID();
        let id: string;
        if (record) throw new Error("This database already has a Storage destination.");
        const empty = await database.initialRestoreIsEligible();
        if (intent.mode === "first-restore") {
          if (!empty || (await hasLegacyLocalVaults()))
            throw new Error("Merge required: this browser has local history. No data was changed.");
          requireSession();
          if ((await findRestoreResource(expected, signal)) !== intent.id)
            throw new Error("The remote Resource changed. Find it again before restoring.");
          id = intent.id;
        } else {
          if (empty) throw new Error("There is no local database to upload.");
          requireSession();
          if (readReceipt(expected))
            throw new BindingSaveError("Storage confirmed publication. Retry the local save.");
          const resource = await client.ensureDatabase({ creationKey, name: resourceName }, signal);
          requireSession();
          id = resource.id;
          intent.onResource(id);
        }
        requireSession();
        const binding = { namespace: owner, id, confirmationId };
        pending.sync = createSqlResourceSync(database, resourceIdentity(id), {
          ...(intent.mode === "first-restore"
            ? { mode: intent.mode, localTag: encodeBinding(binding) }
            : { mode: intent.mode }),
          storageApiUrl: expected.storage,
          async tokenProvider(attempt) {
            requireSession();
            const token = await client.getStorageToken(
              "storage:sync",
              intent.mode === "first-restore" ? ["read"] : ["read", "write"],
              attempt
            );
            requireSession();
            return token;
          },
        });
        await pending.sync.sync();
        requireSession();
        if (intent.mode === "first-upload") {
          if (!authState.hasVerifiedToken(expected)) throw new Error("Sign-in session ended");
          retainReceipt({
            ...binding,
            version: 1,
            loginState: expected.loginState,
            expected: null,
          });
          await savePublishedBinding(database, expected);
        }
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
  const previous = attachment;
  attachment = undefined;
  syncStatus = undefined;
  if (previous) {
    const closed = previous.sync.close().finally(() => {
      previous.database.close();
      if (closingAttachment === closed) closingAttachment = undefined;
    });
    closingAttachment = closed;
  }
  return Promise.all([active?.sync?.close(), closingAttachment]);
}

function watchDetach() {
  const channel = new BroadcastChannel(bindingLock);
  channel.onmessage = () => void cancelExchange();
  return () => channel.close();
}

function attachmentKey(owner: string) {
  return `${bindingLock}:attached:${owner}`;
}

function attachmentPermission(expected: Session, revision: string) {
  return JSON.stringify([1, expected.loginState, revision]);
}

function isAttached(binding: { namespace: string; revision: string }, expected: Session) {
  const key = attachmentKey(binding.namespace);
  try {
    return localStorage.getItem(key) === attachmentPermission(expected, binding.revision);
  } catch {
    return false;
  }
}

function attach(expected: Session, revision: string) {
  const key = attachmentKey(namespace(expected));
  localStorage.setItem(key, attachmentPermission(expected, revision));
}

function detach(expected: Session) {
  void cancelExchange();
  const key = attachmentKey(namespace(expected));
  const cleared = clearLocal(key);
  clearPendingConfirmation(expected);
  const channel = new BroadcastChannel(bindingLock);
  try {
    channel.postMessage("detach");
  } finally {
    channel.close();
  }
  return cleared;
}

async function setDetached(expected: Session, id: string, detached: boolean) {
  if (authState.session !== expected) throw new Error("Sign-in session changed.");
  if (detached) {
    if (!detach(expected))
      throw new Error("Could not save detachment. Sync stays disabled in this tab.");
    return;
  }
  if (attachment) {
    if (attachment.session === expected && attachment.id === id) return;
    throw new Error("Detach the active Storage sync before reattaching.");
  }
  if (active) throw new Error("A sync exchange is already in progress");
  const pending: Exchange = { controller: new AbortController() };
  active = pending;
  const signal = AbortSignal.any([pending.controller.signal, AbortSignal.timeout(bindingTimeout)]);
  try {
    await closingAttachment;
    await navigator.locks.request(bindingLock, { signal }, async () => {
      const database = await openSharedDatabase();
      try {
        const record = await readBinding(database);
        signal.throwIfAborted();
        if (!authState.hasVerifiedToken(expected)) throw new Error("Sign-in session changed.");
        if (!record || record.id !== id || record.namespace !== namespace(expected))
          throw new Error("The Storage binding changed. Check the destination before reattaching.");
        const { client } = storageAccess(expected);
        // Saved permission alone is never authority to start in a new runtime.
        attach(expected, record.revision);
        const sync = startSqlResourceSync(database, resourceIdentity(record.id), {
          storageApiUrl: expected.storage,
          async tokenProvider(attempt) {
            const requireAttachment = () => {
              attempt?.throwIfAborted();
              if (
                attachment?.sync !== sync ||
                authState.session !== expected ||
                !isAttached(record, expected)
              )
                throw new Error("Storage sync is detached.");
            };
            requireAttachment();
            const token = await client.getStorageToken("storage:sync", ["read", "write"], attempt);
            requireAttachment();
            return token;
          },
          onStatus(status) {
            if (attachment?.sync === sync) syncStatus = status;
          },
        });
        syncStatus = { status: "syncing" };
        attachment = { session: expected, id: record.id, sync, database };
      } finally {
        if (attachment?.database !== database) database.close();
      }
    });
  } finally {
    active = undefined;
  }
}

async function signOut(expected: Session) {
  if (authState.session !== expected) return;
  try {
    detach(expected);
  } finally {
    await authState.logout();
  }
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
  retryBindingSave,
  clearPendingConfirmation,
  watchDetach,
  setDetached,
  signOut,
  getSyncStatus,
  syncBoundDatabase(session: Session) {
    if (attachment?.session !== session) throw new Error("Storage sync is detached.");
    return attachment.sync.sync();
  },
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
