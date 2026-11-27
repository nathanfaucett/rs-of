import { SyncClient, SyncError } from "@ofsdk/storage-sync-client";
import {
  createSqlResourceSync,
  type SqlResourceSync,
  type SqlResourceSyncStatus,
  startSqlResourceSync,
} from "@ofsdk/sync-sdk/browser";
import { authState } from "$lib/auth/auth.svelte";
import type { VaultHandle } from "$lib/vault/encrypted-vault";
import { legacyCreationKey, openLegacyReplica, resourceIdPattern } from "$lib/vault/legacy-history";
import {
  archiveMerge,
  clearMerge,
  decodeRevision,
  encodeRevision,
  exchangeHistory,
  type MergeDestination,
  type PendingMerge,
  pullMergeSnapshot,
  readPendingMerge,
  reopenCandidate,
  retainCandidate,
  retainMerge,
} from "$lib/vault/merge-history";
import { hasLegacyLocalVaults, openSharedDatabase } from "$lib/vault/storage/migrate-to-shared";
import { StorageClient } from "../../../../../../packages/storage-client/src/index";

type Session = NonNullable<typeof authState.session>;

type Exchange = { controller: AbortController; sync?: SqlResourceSync };
let active: Exchange | undefined;
const bindingLock = "password-manager-v1:storage-binding";
const bindingTimeout = 2_000;
const receiptKey = `${bindingLock}:receipt`;
let receiptInMemory: Receipt | null | undefined;

type Binding = { namespace: string; id: string; confirmationId: string; legacyVaults?: string[] };
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

function storageAccess(session = authState.session, checkActive?: () => boolean) {
  if (!session || session !== authState.session) throw new Error("Sign in to sync first");
  const requireActive = () => {
    if (session !== authState.session || (checkActive && !checkActive()))
      throw new Error("Sign-in session ended");
  };
  const getToken = async (signal?: AbortSignal) => {
    requireActive();
    const token = await authState.getAccessToken(session, signal);
    requireActive();
    return token;
  };
  const client = new StorageClient({
    idpBaseUrl: session.idp,
    storageBaseUrl: session.storage,
    clientId: session.clientId,
    bearerToken: getToken,
    fetch: (input, init) => {
      requireActive();
      return fetch(input, { ...init, credentials: "omit", redirect: "error" });
    },
  });
  return { session, client, getToken };
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
    JSON.stringify([
      binding.namespace,
      binding.id,
      binding.confirmationId,
      ...(binding.legacyVaults ? [binding.legacyVaults] : []),
    ])
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
      (value.length < 3 ||
        value.length > 4 ||
        (typeof value[2] !== "boolean" && (typeof value[2] !== "string" || !value[2])))) ||
    (value.length === 4 &&
      (typeof value[2] !== "string" ||
        !Array.isArray(value[3]) ||
        !value[3].every((id: unknown) => typeof id === "string" && resourceIdPattern.test(id))))
  )
    throw new Error("Invalid local Storage binding. Your vaults are still local.");
  return {
    namespace: value[0],
    id: value[1],
    revision: typeof value[2] === "string" ? value[2] : revision,
    legacyVaults: value.length === 4 ? (value[3] as string[]) : [],
    tag,
  };
}

async function verifyUpgradedVault(vault: VaultHandle) {
  const database = await openSharedDatabase();
  try {
    const record = await readBinding(database);
    if (!record) return;
    if (!record.legacyVaults.includes(vault.id)) return;
    await vault.readItems();
    await navigator.locks.request(bindingLock, async () => {
      const current = await readBinding(database);
      if (!current || current.revision !== record.revision)
        throw new Error("The Storage binding changed. Unlock again to verify the upgraded vault.");
      if (!current.legacyVaults.includes(vault.id)) return;
      const saved = await database.replaceInitialRestoreTag(
        current.tag,
        encodeBinding({
          namespace: current.namespace,
          id: current.id,
          confirmationId: current.revision,
          legacyVaults: current.legacyVaults.filter((id) => id !== vault.id),
        })
      );
      if (!saved) throw new Error("Could not save upgrade verification. Unlock again to retry.");
    });
  } finally {
    database.close();
  }
}

async function getBinding(expected: Session) {
  const owner = namespace(expected);
  const database = await openSharedDatabase();
  try {
    const record = await readBinding(database);
    if (!record) {
      const merge = readPendingMerge();
      if (merge)
        return ownsMerge(expected, merge)
          ? ({ status: "pending-merge", id: merge.id, committed: merge.committed } as const)
          : ({ status: "merge-session-changed" } as const);
      const receipt = readReceipt(expected);
      return receipt
        ? ({ status: "pending-save", id: receipt.id } as const)
        : ({ status: "unbound" } as const);
    }
    if (record.namespace !== owner) return { status: "different-account" } as const;
    return {
      status: "bound",
      id: record.id,
      unverified: record.legacyVaults.length,
    } as const;
  } finally {
    database.close();
  }
}

async function findRestoreResource(
  expected: Session,
  signal: AbortSignal,
  checkActive: () => boolean,
  key = creationKey
) {
  const resource = await storageAccess(expected, checkActive).client.findSqlDatabaseByCreationKey(
    key,
    signal
  );
  if (!checkActive()) throw new Error("Sign-in session ended");
  return resource?.id;
}

async function isPristineForRestore(database: Database) {
  return (await database.initialRestoreIsEligible()) && !(await hasLegacyLocalVaults());
}

type RestoreInspection =
  | { id: string; mode: "restore" | "legacy" }
  | (MergeDestination & { mode: "merge" | "upgrade" });

async function verifyRecoveryResource(expected: Session, id: string, signal?: AbortSignal) {
  const { client } = storageAccess(expected);
  if (
    !resourceIdPattern.test(id) ||
    !(await client.listSqlDatabases(signal)).some((entry) => entry.id === id)
  )
    throw new Error("This exact Storage Resource is not available in the signed-in namespace.");
  if (!authState.hasVerifiedToken(expected)) throw new Error("Sign-in session ended");
}

async function legacyBrowserRevision(id: string) {
  const database = await openLegacyReplica(id);
  try {
    return database ? encodeRevision(await database.historyRevision()) : null;
  } finally {
    database?.close();
  }
}

async function inspectRestore(
  expected: Session,
  explicitId?: string
): Promise<RestoreInspection | undefined> {
  return withExchange(expected, async (database, signal, checkActive) => {
    requireUnbound(await readBinding(database));
    if (readPendingMerge()) throw new Error("A merge is already pending.");
    const current = explicitId
      ? undefined
      : await findRestoreResource(expected, signal, checkActive);
    const id =
      current ??
      explicitId ??
      (await findRestoreResource(expected, signal, checkActive, legacyCreationKey));
    if (!id) return;
    if (explicitId) await verifyRecoveryResource(expected, id, signal);
    if (await hasLegacyLocalVaults()) return { id, mode: "legacy" };
    if (!checkActive()) throw new Error("Sign-in session ended");
    if (current && (await database.initialRestoreIsEligible())) return { id, mode: "restore" };
    const local = encodeRevision(await database.historyRevision());
    const legacy = current
      ? undefined
      : { browser: await legacyBrowserRevision(id), explicit: !!explicitId };
    const { client, attach } = mergeConnection(expected, id, signal, checkActive, false);
    const remote = await pullMergeSnapshot(client, attach, (_snapshot, pinned) => pinned);
    if (!checkActive()) throw new Error("Sign-in session ended");
    return legacy
      ? { id, local, remote, legacy, mode: "upgrade" }
      : { id, local, remote, mode: "merge" };
  });
}

function requireUnbound(record: Awaited<ReturnType<typeof readBinding>>) {
  if (record) throw new Error("This database already has a Storage destination.");
}

function ownsMerge(expected: Session, merge: PendingMerge) {
  return (
    merge.namespace === namespace(expected) &&
    (merge.legacy !== undefined ||
      (merge.loginState === expected.loginState && merge.clientId === expected.clientId))
  );
}

function mergeConnection(
  expected: Session,
  id: string,
  signal: AbortSignal,
  checkActive: () => boolean,
  write: boolean
) {
  const { client } = storageAccess(expected, checkActive);
  return {
    client: new SyncClient({
      storageApiUrl: expected.storage,
      tokenProvider: (attempt) =>
        client.getStorageToken("storage:sync", write ? ["read", "write"] : ["read"], attempt),
    }),
    attach: {
      identity: resourceIdentity(id),
      signal: AbortSignal.any([signal, AbortSignal.timeout(135_000)]),
      checkActive,
    },
  };
}

function publicationFor(merge: PendingMerge) {
  return {
    operation: merge.operation,
    expected: decodeRevision(merge.remote),
    candidate: decodeRevision(merge.candidate),
  };
}

async function checkMergeReceipt(
  merge: PendingMerge,
  client: SyncClient,
  attach: ReturnType<typeof mergeConnection>["attach"]
) {
  if (merge.attempted === false) return false;
  const committed = await client.checkPublication({
    ...attach,
    publication: publicationFor(merge),
  });
  if (!attach.checkActive() || attach.signal.aborted) throw new Error("Sync cancelled");
  if (merge.committed && !committed)
    throw new Error("The prior Storage receipt is no longer available. No history was sent.");
  if (committed && !merge.committed) retainMerge({ ...merge, committed });
  return committed;
}

async function reviewPendingMerge(expected: Session) {
  await withExchange(expected, async (database, signal, checkActive) => {
    requireUnbound(await readBinding(database));
    const merge = readPendingMerge();
    if (!merge || !ownsMerge(expected, merge))
      throw new Error("This session cannot review the pending merge.");
    const localMatches = await matchesLocalRevision(database, merge);
    const write = mergeConnection(expected, merge.id, signal, checkActive, true);
    let committed = await checkMergeReceipt(merge, write.client, write.attach);
    if (!committed) {
      const pull = mergeConnection(expected, merge.id, signal, checkActive, false);
      const remote = await pullMergeSnapshot(
        pull.client,
        pull.attach,
        (_snapshot, pinned) => pinned
      );
      if (remote === merge.remote) {
        if (localMatches)
          throw new Error("The histories still match. Retry the exact pending merge.");
        if (merge.attempted !== false)
          throw new Error(
            "Local history changed, but Storage may still commit the pending operation. No history was sent. Check again later."
          );
      } else {
        // The old compare cannot start from this newer history. Check for a receipt after Pull.
        committed = await checkMergeReceipt(merge, write.client, write.attach);
      }
    }
    if (committed && localMatches)
      throw new Error("Storage confirmed publication. Retry the pending merge to save it locally.");
    if (!checkActive()) throw new Error("Sign-in session ended");
    archiveMerge({ ...merge, committed });
  });
}

async function mergeDatabases(expected: Session, destination?: MergeDestination) {
  await withExchange(expected, async (database, signal, checkActive) => {
    requireUnbound(await readBinding(database));
    let merge = readPendingMerge();
    const id = destination?.id ?? merge?.id;
    if (!destination && (!merge || !ownsMerge(expected, merge)))
      throw new Error("The sign-in session changed. This session cannot resume the merge.");
    const legacy = (destination ?? merge)?.legacy;
    if (!id) throw new Error("The remote Resource changed. No merge was applied.");
    if (legacy?.explicit) await verifyRecoveryResource(expected, id, signal);
    else if (
      (await findRestoreResource(
        expected,
        signal,
        checkActive,
        legacy ? legacyCreationKey : creationKey
      )) !== id
    )
      throw new Error("The remote Resource changed. No merge was applied.");
    if (!checkActive()) throw new Error("Sign-in session ended");
    if (destination) {
      if (merge || readReceipt(expected)) throw new Error("Resolve the pending operation first.");
      await requireLocalRevision(database, destination, false);
      const { client, attach } = mergeConnection(expected, id, signal, checkActive, false);
      await pullMergeSnapshot(client, attach, async (snapshot, revision) => {
        if (revision !== destination.remote)
          throw new Error("Remote history changed. Find the remote database again before merging.");
        const browser = legacy ? await openLegacyReplica(id) : undefined;
        try {
          const captured = browser ? encodeRevision(await browser.historyRevision()) : null;
          if (legacy && legacy.browser !== captured)
            throw new Error(
              "The older browser copy changed. Find the Resource again before upgrading."
            );
          merge = await retainCandidate(
            database,
            snapshot,
            {
              id,
              local: destination.local,
              remote: destination.remote,
              ...(legacy && { legacy }),
              version: 1,
              operation: crypto.randomUUID(),
              committed: false,
              attempted: false,
              namespace: namespace(expected),
              loginState: expected.loginState,
              clientId: expected.clientId,
            },
            checkActive,
            browser
          );
        } finally {
          browser?.close();
        }
      });
    }
    if (!merge || !ownsMerge(expected, merge))
      throw new Error("The sign-in session changed. This session cannot resume the merge.");
    const publication = publicationFor(merge);
    const { client, attach } = mergeConnection(expected, id, signal, checkActive, true);
    let committed = !destination && (await checkMergeReceipt(merge, client, attach));
    await requireLocalRevision(database, merge, committed);
    if (!committed && !destination) {
      const pendingMerge = merge;
      const pull = mergeConnection(expected, id, signal, checkActive, false);
      await pullMergeSnapshot(pull.client, pull.attach, async (_snapshot, revision) => {
        if (revision !== pendingMerge.remote) {
          // A prior compare can commit between the first receipt check and this Pull.
          if (!(await checkMergeReceipt(pendingMerge, client, attach)))
            throw new Error(
              "Remote history changed. Review the pending merge before giving fresh consent."
            );
          committed = true;
        }
      });
    }
    const stage = await reopenCandidate(merge);
    try {
      if (!committed && (!merge.legacy || merge.candidate !== merge.remote)) {
        try {
          await requireLocalRevision(database, merge, false);
          if (!checkActive()) throw new Error("Sign-in session ended");
          merge = { ...merge, attempted: true };
          retainMerge(merge);
          const session = await client.attachComparePublish({ ...attach, publication });
          try {
            await requireLocalRevision(database, merge, false);
            await exchangeHistory(stage, session, checkActive);
            await session.waitForPublish();
          } catch (cause) {
            throw session.failure ?? cause;
          } finally {
            session.close();
          }
        } catch (cause) {
          // A lost receipt or an overlapping attempt can still have committed this operation.
          if (cause instanceof SyncError && cause.code === "destination-changed")
            throw new Error(
              "Remote history changed. Storage refused the merge. Review the pending merge before giving fresh consent.",
              { cause }
            );
          throw cause;
        }
        retainMerge({ ...merge, committed: true });
        committed = true;
      }
      try {
        await requireLocalRevision(database, merge, committed);
        await database.publishLocalHistory(
          stage,
          decodeRevision(merge.local),
          publication.candidate,
          encodeBinding({
            namespace: merge.namespace,
            id,
            confirmationId: merge.operation,
            ...(merge.legacyVaults && { legacyVaults: merge.legacyVaults }),
          }),
          checkActive
        );
      } catch (cause) {
        await requireLocalRevision(database, merge, committed);
        throw new Error(
          committed
            ? "Storage committed the merge, but the local merge was not saved. Retry the pending merge."
            : "The Storage snapshot was not saved locally. Retry the pending merge.",
          { cause }
        );
      }
    } finally {
      stage.close();
    }
    clearMerge(merge);
  });
}

async function matchesLocalRevision(database: Database, expected: MergeDestination) {
  return (
    !(await database.initialRestoreTag()) &&
    !(await hasLegacyLocalVaults()) &&
    encodeRevision(await database.historyRevision()) === expected.local &&
    (!expected.legacy || (await legacyBrowserRevision(expected.id)) === expected.legacy.browser)
  );
}

async function requireLocalRevision(
  database: Database,
  expected: MergeDestination,
  committed: boolean
) {
  if (!(await matchesLocalRevision(database, expected)))
    throw new Error(
      committed
        ? "Storage committed the merge, but this browser changed. Your current local data was kept."
        : "Local history changed. This merge cannot send or replace your current local data."
    );
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

async function withExchange<T>(
  expected: Session,
  run: (
    database: Database,
    signal: AbortSignal,
    checkActive: () => boolean,
    pending: Exchange
  ) => Promise<T>
) {
  if (authState.session !== expected) throw new Error("Sign-in session ended");
  if (active || attachment)
    throw new Error("Detach Storage sync before starting an initial exchange.");
  const pending: Exchange = { controller: new AbortController() };
  active = pending;
  const { signal } = pending.controller;
  const checkActive = () => !signal.aborted && authState.hasVerifiedToken(expected);
  try {
    await closingAttachment;
    return await navigator.locks.request(bindingLock, { signal }, async () => {
      if (!checkActive()) throw new Error("Sign-in session ended");
      const database = await openSharedDatabase();
      try {
        if (!checkActive()) throw new Error("Sign-in session ended");
        return await run(database, signal, checkActive, pending);
      } finally {
        await pending.sync?.close();
        database.close();
      }
    });
  } finally {
    active = undefined;
  }
}

async function exchange(expected: Session, intent: Intent) {
  await withExchange(expected, async (database, signal, checkActive, pending) => {
    function requireSession() {
      if (!checkActive()) throw new Error("Sign-in session ended");
    }
    const { client } = storageAccess(expected, checkActive);
    const record = await readBinding(database);
    const owner = namespace(expected);
    const confirmationId = crypto.randomUUID();
    let id: string;
    requireUnbound(record);
    if (readPendingMerge()) throw new Error("Resolve the pending merge first.");
    if (intent.mode === "first-restore") {
      if (!(await isPristineForRestore(database)))
        throw new Error("Merge required: this browser has local history. No data was changed.");
      requireSession();
      if ((await findRestoreResource(expected, signal, checkActive)) !== intent.id)
        throw new Error("The remote Resource changed. Find it again before restoring.");
      id = intent.id;
    } else {
      if (await hasLegacyLocalVaults())
        throw new Error("Unlock the older local vaults before uploading this database.");
      if (await database.initialRestoreIsEligible())
        throw new Error("There is no local database to upload.");
      requireSession();
      if (readReceipt(expected))
        throw new BindingSaveError("Storage confirmed publication. Retry the local save.");
      if (await findRestoreResource(expected, signal, checkActive, legacyCreationKey))
        throw new Error(
          "This account has an older database. Find the remote database and review the upgrade first."
        );
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
      checkActive: () => !signal.aborted && authState.hasVerifiedToken(expected),
      tokenProvider: (attempt) =>
        client.getStorageToken(
          "storage:sync",
          intent.mode === "first-restore" ? ["read"] : ["read", "write"],
          attempt
        ),
    });
    await pending.sync.sync();
    requireSession();
    if (intent.mode === "first-upload") {
      retainReceipt({
        ...binding,
        version: 1,
        loginState: expected.loginState,
        expected: null,
      });
      await savePublishedBinding(database, expected);
    }
  });
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
        if (record.legacyVaults.length)
          throw new Error(
            "Unlock each upgraded vault to verify its contents before reattaching sync."
          );
        const checkActive = (): boolean =>
          attachment?.sync === sync &&
          authState.hasVerifiedToken(expected) &&
          isAttached(record, expected);
        const { client } = storageAccess(expected, checkActive);
        // Saved permission alone is never authority to start in a new runtime.
        attach(expected, record.revision);
        const sync = startSqlResourceSync(database, resourceIdentity(record.id), {
          storageApiUrl: expected.storage,
          runExclusive: async (run, signal) => {
            await navigator.locks.request(bindingLock, { signal }, async () => {
              const current = await readBinding(database);
              if (
                current?.namespace !== record.namespace ||
                current.id !== record.id ||
                current.revision !== record.revision
              )
                throw new Error("The Storage binding changed. Reattach before syncing.");
              await run();
            });
          },
          checkActive,
          tokenProvider: (attempt) =>
            client.getStorageToken("storage:sync", ["read", "write"], attempt),
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
  await cancelExchange();
}

export const syncStateModule = {
  closeSync,
  inspectRestore,
  verifyUpgradedVault,
  mergeDatabases,
  reviewPendingMerge,
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
};
