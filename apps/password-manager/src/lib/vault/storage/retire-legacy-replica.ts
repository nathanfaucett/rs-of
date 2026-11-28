const stores = ["meta", "tables", "records"];
const format = "ofdb-byte-storage-1";
export const retiredRevision = "18446744073709551615";

function accessError(cause?: unknown) {
  return new Error(
    "Cannot access the validated older browser revision. Keep browser data and retry.",
    {
      cause,
    }
  );
}

function openExisting(name: string): Promise<IDBDatabase | undefined> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(`ofdb:sql:${name}`, 1);
    let missing = false;
    let refused = false;
    request.onblocked = () => {
      refused = true;
      reject(accessError());
    };
    request.onupgradeneeded = (event) => {
      missing = event.oldVersion === 0;
      request.transaction?.abort();
    };
    request.onerror = () => {
      if (missing && !refused) resolve(undefined);
      else reject(accessError(request.error));
    };
    request.onsuccess = () => {
      if (refused) request.result.close();
      else resolve(request.result);
    };
  });
}

function revisionTransaction(
  db: IDBDatabase,
  expected?: string,
  canRetire = () => true
): Promise<string> {
  return new Promise((resolve, reject) => {
    if (
      db.objectStoreNames.length !== stores.length ||
      stores.some((name) => !db.objectStoreNames.contains(name))
    )
      throw accessError();
    let tx: IDBTransaction;
    try {
      tx = db.transaction(stores, expected === undefined ? "readonly" : "readwrite", {
        durability: "strict",
      });
    } catch (cause) {
      throw accessError(cause);
    }
    let revision: string;
    tx.onabort = () => reject(accessError(tx.error));
    tx.oncomplete = () => resolve(revision);
    for (const name of stores) {
      const store = tx.objectStore(name);
      if (store.keyPath !== null || store.autoIncrement || store.indexNames.length) {
        tx.abort();
        return;
      }
    }
    const meta = tx.objectStore("meta");
    const marker = meta.get("format");
    const kind = meta.get("kind");
    const read = meta.get("revision");
    read.onsuccess = () => {
      const value: unknown = read.result;
      if (
        marker.result !== format ||
        kind.result !== "sql" ||
        typeof value !== "string" ||
        !/^(0|[1-9][0-9]*)$/.test(value) ||
        BigInt(value) > BigInt(retiredRevision) ||
        (expected !== undefined && (value !== expected || !canRetire()))
      ) {
        tx.abort();
        return;
      }
      revision = value;
      if (expected !== undefined && value !== retiredRevision)
        meta.put(retiredRevision, "revision");
    };
  });
}

export async function openLegacyReplicaFence(name: string) {
  const db = await openExisting(name);
  if (!db) return;
  // A replacement database can start at the same revision. Do not reopen for the CAS.
  db.onversionchange = () => db.close();
  try {
    const revision = await revisionTransaction(db);
    return {
      revision,
      close: () => db.close(),
      // Older WASM writers compare this revision in their own readwrite transaction.
      retire: async (canRetire?: () => boolean) => {
        await revisionTransaction(db, revision, canRetire);
      },
    };
  } catch (error) {
    db.close();
    throw error;
  }
}
