/**
 * Create an incompatible schema or metadata fixture containing a preservation sentinel.
 * @param {string} name - Fresh logical database name.
 * @param {string} kind - Storage kind used in the physical name and metadata.
 * @param {'version'|'missing'|'keypath'|'autoincrement'|'index'|'extra'|'format'|'missing-kind'|'kind'|'revision'} mode - Incompatibility to create.
 */
export async function malformed(name, kind, mode) {
  const db = await new Promise((resolve, reject) => {
    const request = indexedDB.open(`ofdb:${kind}:${name}`, mode === "version" ? 2 : 1);
    request.onupgradeneeded = () => {
      for (const store of ["meta", "tables", "records"]) {
        if (mode === "missing" && store === "records") continue;
        const options =
          mode === "keypath" && store === "records"
            ? { keyPath: "secret" }
            : mode === "autoincrement" && store === "records"
              ? { autoIncrement: true }
              : undefined;
        const objectStore = request.result.createObjectStore(store, options);
        if (mode === "index" && store === "records") {
          objectStore.createIndex("foreign-index", "secret");
        }
      }
      if (mode === "extra") request.result.createObjectStore("foreign-store");
      const meta = request.transaction.objectStore("meta");
      meta.put(mode === "format" ? "future" : "ofdb-byte-storage-1", "format");
      if (mode !== "missing-kind") meta.put(mode === "kind" ? "wrong" : kind, "kind");
      meta.put(mode === "revision" ? "18446744073709551616" : "0", "revision");
      meta.put("preserve-me", "sentinel");
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  db.close();
}

/**
 * Read the sentinel to verify a rejected open preserved the fixture's metadata.
 * @param {string} name - Logical fixture database name.
 * @param {string} kind - Storage kind used in the physical database name.
 * @returns {Promise<string|undefined>} Stored sentinel, or undefined if absent.
 */
export async function sentinel(name, kind) {
  const request = indexedDB.open(`ofdb:${kind}:${name}`);
  const db = await new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  const value = await new Promise((resolve, reject) => {
    const tx = db.transaction("meta");
    const result = tx.objectStore("meta").get("sentinel");
    tx.oncomplete = () => resolve(result.result);
    tx.onabort = () => reject(tx.error);
  });
  db.close();
  return value;
}

/** Make the next native open throw SecurityError containing a test secret. */
export function denyOpen() {
  const original = IDBFactory.prototype.open;
  IDBFactory.prototype.open = () => {
    IDBFactory.prototype.open = original;
    throw new DOMException("private-key-and-value-must-not-leak", "SecurityError");
  };
}
