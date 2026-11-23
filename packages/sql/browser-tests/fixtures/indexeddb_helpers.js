/**
 * Inject a records-store commit failure through the native put prototype.
 * Restores put before aborting after the first successful request or throwing on
 * the second request, depending on mode. Only the test harness uses this hook.
 * @param {'abort'|'throw'} mode - Abort the transaction or throw QuotaExceededError.
 */
export function failNextCommit(mode) {
  const original = IDBObjectStore.prototype.put;
  let count = 0;
  IDBObjectStore.prototype.put = function (...args) {
    if (this.name !== "records") return original.apply(this, args);
    count++;
    if (mode === "throw" && count === 2) {
      IDBObjectStore.prototype.put = original;
      throw new DOMException("test quota failure", "QuotaExceededError");
    }
    const request = original.apply(this, args);
    if (mode === "abort") {
      IDBObjectStore.prototype.put = original;
      const tx = this.transaction;
      request.addEventListener("success", () => tx.abort());
    }
    return request;
  };
}
