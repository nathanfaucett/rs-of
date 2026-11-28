const stores = ["meta", "tables", "records"];
const format = "ofdb-byte-storage-1";
const retired = "18446744073709551615";

// The legacy WASM writer checks this u64 revision in its own readwrite transaction.
// Version 1 remains readable; stale and freshly opened writers both fail to commit.
export function retireLegacyReplica(name: string, reserveMissing = false): Promise<void> {
  return new Promise((resolve, reject) => {
    const failure = (cause?: unknown) =>
      new Error(
        "Cannot retire the older browser database safely. Keep browser data and retry with a compatible app.",
        { cause }
      );
    const request = indexedDB.open(`ofdb:sql:${name}`, 1);
    let refused = false;
    let db: IDBDatabase | undefined;
    const timer = setTimeout(() => fail(), 5_000);
    function fail(cause?: unknown) {
      refused = true;
      clearTimeout(timer);
      db?.close();
      reject(failure(cause));
    }
    request.onblocked = () => fail();
    request.onerror = () => fail(request.error);
    request.onupgradeneeded = () => {
      const tx = request.transaction;
      if (!tx) return;
      if (refused || !reserveMissing) {
        tx.abort();
        return;
      }
      // Reserve the absent namespace too: a cached client must not create a writable replica later.
      for (const store of stores) request.result.createObjectStore(store);
      const meta = tx.objectStore("meta");
      meta.put(format, "format");
      meta.put("sql", "kind");
      meta.put(retired, "revision");
    };
    request.onsuccess = () => {
      db = request.result;
      if (refused) {
        db.close();
        return;
      }
      db.onversionchange = () => db?.close();
      try {
        if (
          db.objectStoreNames.length !== stores.length ||
          stores.some((name) => !db?.objectStoreNames.contains(name))
        ) {
          fail();
          return;
        }
        const tx = db.transaction(stores, "readwrite", { durability: "strict" });
        tx.onabort = () => fail(tx.error);
        tx.oncomplete = () => {
          clearTimeout(timer);
          db?.close();
          if (!refused) resolve();
        };
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
        const revision = meta.get("revision");
        revision.onsuccess = () => {
          const value: unknown = revision.result;
          if (
            refused ||
            marker.result !== format ||
            kind.result !== "sql" ||
            typeof value !== "string" ||
            !/^(0|[1-9][0-9]*)$/.test(value) ||
            BigInt(value) > BigInt(retired)
          ) {
            tx.abort();
          } else if (value !== retired) {
            meta.put(retired, "revision");
          }
        };
      } catch (cause) {
        fail(cause);
      }
    };
  });
}
