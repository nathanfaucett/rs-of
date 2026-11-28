import { Database, JsSqlSyncTransport } from "@ofsdk/sql";
import type { AttachOptions, SyncClient, SyncSession } from "@ofsdk/storage-sync-client";
import { vaultHistoryRecords } from "./history-records";
import { type captureLegacyReplica, resourceIdPattern, stageLegacyUpgrade } from "./legacy-history";
import { MIGRATIONS } from "./storage/schema";
import { validateVaultDatabase } from "./storage/validate-vault-database";

const mergeKey = "password-manager-v1:storage-binding:merge";

const revision = /^[\da-f]{64}$/;

export type MergeDestination = {
  id: string;
  local: string;
  remote: string;
  legacy?: { browser: string | null; explicit: boolean };
};
export type PendingMerge = MergeDestination & {
  version: 1;
  namespace: string;
  loginState: string;
  clientId: string;
  operation: string;
  candidate: string;
  legacyVaults?: string[];
  validatedLegacy?: true;
  committed: boolean;
  // Missing on older pending operations: treat those as possibly sent.
  attempted?: boolean;
};

export function readPendingMerge(): PendingMerge | undefined {
  const raw = localStorage.getItem(mergeKey);
  if (raw === null) return;
  const value = JSON.parse(raw);
  if (
    value?.version !== 1 ||
    ![value.id, value.operation].every(
      (id) => typeof id === "string" && resourceIdPattern.test(id)
    ) ||
    (value.legacy !== undefined &&
      (!value.legacy ||
        typeof value.legacy.explicit !== "boolean" ||
        (value.legacy.browser !== null &&
          (typeof value.legacy.browser !== "string" || !revision.test(value.legacy.browser))) ||
        !Array.isArray(value.legacyVaults) ||
        !value.legacyVaults.every(
          (id: unknown) => typeof id === "string" && resourceIdPattern.test(id)
        ))) ||
    ![value.local, value.remote, value.candidate].every(
      (hash) => typeof hash === "string" && revision.test(hash)
    ) ||
    ![value.namespace, value.clientId, value.loginState].every(
      (text) => typeof text === "string" && text.length > 0
    ) ||
    typeof value.committed !== "boolean" ||
    (value.attempted !== undefined && typeof value.attempted !== "boolean") ||
    (value.committed && value.attempted === false)
  )
    throw new Error("Invalid pending merge. Your vaults are still local.");
  return value;
}

export function retainMerge(merge: PendingMerge) {
  localStorage.setItem(mergeKey, JSON.stringify(merge));
}

export function encodeRevision(bytes: Uint8Array) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function decodeRevision(hash: string) {
  return Uint8Array.from(hash.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

export async function exchangeHistory(
  database: Database,
  session: SyncSession,
  checkActive: () => boolean
) {
  await database.synchronizeHistory(
    new JsSqlSyncTransport({
      async send(bytes: Uint8Array) {
        if (!checkActive()) throw new Error("Sign-in session ended");
        session.send(bytes);
      },
      receive: () => session.receive(),
    }),
    undefined,
    () => !session.failure && checkActive()
  );
}

export async function pullMergeSnapshot<T>(
  client: SyncClient,
  attach: Omit<AttachOptions, "direction"> & { checkActive: () => boolean },
  useSnapshot: (database: Database, revision: string) => T | Promise<T>
) {
  // The Pull initiator is empty. No live local handle participates in this exchange.
  const snapshot = await Database.inMemory();
  try {
    const session = await client.attachRevisionedPull(attach);
    try {
      await exchangeHistory(snapshot, session, attach.checkActive);
      const pinned = encodeRevision(await session.waitForSnapshotRevision());
      if (encodeRevision(await snapshot.historyRevision()) !== pinned)
        throw new Error("The remote snapshot did not match its history revision.");
      if (!attach.checkActive() || attach.signal?.aborted) throw new Error("Sync cancelled");
      return await useSnapshot(snapshot, pinned);
    } finally {
      session.close();
    }
  } finally {
    snapshot.close();
  }
}

function candidateName(operation: string) {
  return `${mergeKey}:${operation}`;
}

export async function retainCandidate(
  local: Database,
  remote: Database,
  merge: Omit<PendingMerge, "candidate">,
  checkActive: () => boolean,
  browser?: NonNullable<Awaited<ReturnType<typeof captureLegacyReplica>>>
) {
  const { stage, vaults } = merge.legacy
    ? await stageLegacyUpgrade(local, remote, merge.legacy.explicit, browser?.database)
    : { stage: await local.stageHistory(remote, vaultHistoryRecords), vaults: undefined };
  try {
    if (merge.legacy) await validateVaultDatabase(stage, true, false);
    const before = await stage.historyRevision();
    const client = stage.client();
    try {
      if ((await client.migrate(MIGRATIONS)).appliedVersions.length !== 0)
        throw new Error(
          "The candidate does not have the complete Password Manager migration history."
        );
    } finally {
      client.close();
    }
    if (encodeRevision(await local.historyRevision()) !== merge.local)
      throw new Error("Local history changed. Find the remote database again before merging.");
    if (!checkActive()) throw new Error("Sign-in session ended");
    await browser?.retire(checkActive);
    const name = candidateName(merge.operation);
    const saved = await Database.open(name);
    let retained = false;
    try {
      await saved.publishLocalHistory(
        stage,
        await saved.historyRevision(),
        before,
        new TextEncoder().encode(merge.operation),
        checkActive
      );
      if (!checkActive()) throw new Error("Sign-in session ended");
      const pending = {
        ...merge,
        candidate: encodeRevision(before),
        ...(vaults && { legacyVaults: vaults, validatedLegacy: true as const }),
      };
      retainMerge(pending);
      retained = true;
      return pending;
    } finally {
      saved.close();
      if (!retained) indexedDB.deleteDatabase(`ofdb:sql:${name}`);
    }
  } finally {
    stage.close();
  }
}

export async function reopenCandidate(merge: PendingMerge) {
  const saved = await Database.open(candidateName(merge.operation));
  try {
    const tag = await saved.initialRestoreTag();
    if (!tag || new TextDecoder().decode(tag) !== merge.operation)
      throw new Error("The saved merge candidate is unavailable. No new history was sent.");
    // Reopen in memory: only a private candidate can participate in compare or local publication.
    const stage = await saved.stageHistory(saved, vaultHistoryRecords);
    try {
      if (encodeRevision(await stage.historyRevision()) !== merge.candidate)
        throw new Error("The saved merge candidate changed. No new history was sent.");
      if (merge.legacy) await validateVaultDatabase(stage, true, false);
      return stage;
    } catch (error) {
      stage.close();
      throw error;
    }
  } finally {
    saved.close();
  }
}

export function clearMerge(merge: PendingMerge) {
  try {
    localStorage.removeItem(mergeKey);
  } catch {
    // The live binding is already committed and takes precedence over pending metadata.
    return;
  }
  indexedDB.deleteDatabase(`ofdb:sql:${candidateName(merge.operation)}`);
}

export function archiveMerge(merge: PendingMerge) {
  // Keep the operation tuple and its private candidate. Only free the active slot.
  localStorage.setItem(candidateName(merge.operation), JSON.stringify(merge));
  localStorage.removeItem(mergeKey);
}
