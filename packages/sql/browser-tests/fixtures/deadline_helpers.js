/**
 * Hold the next matching transaction's oncomplete callback until explicitly released.
 * Native completion remains observable through completed, independently of machine speed.
 * @param {IDBTransactionMode} mode - Transaction mode to intercept once.
 * @returns {{completed: Promise<void>, release: function(): void}} Native completion
 * notification and a release function that also restores the transaction prototype.
 */
export function holdNextCompletion(mode) {
  const original = IDBDatabase.prototype.transaction;
  let release = () => {};
  const released = new Promise((resolve) => {
    release = resolve;
  });
  let complete = () => {};
  const completed = new Promise((resolve) => {
    complete = resolve;
  });
  IDBDatabase.prototype.transaction = function (...args) {
    const tx = original.apply(this, args);
    if (tx.mode !== mode) return tx;
    IDBDatabase.prototype.transaction = original;
    tx.addEventListener("complete", () => complete());
    Object.defineProperty(tx, "oncomplete", {
      /**
       * Defer a completion callback until the hold is released.
       * @param {function(this: IDBTransaction, Event): void} handler - Adapter callback.
       */
      set(handler) {
        tx.addEventListener("complete", (event) => {
          void released.then(() => handler.call(tx, event));
        });
      },
    });
    return tx;
  };
  return {
    completed,
    release() {
      IDBDatabase.prototype.transaction = original;
      release();
    },
  };
}
